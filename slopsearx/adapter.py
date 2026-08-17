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

# Declarative capability vocabulary (design §4.6, capability-catalog feature).
# These are the canonical, closed sets the live capability catalog and the
# filter-enforcement report derive from. Adapters declare membership; the
# catalog normalizes and fills stable defaults so every entry is complete.
SUPPORTED_FILTER_KEYS: tuple[str, ...] = ("language", "time_range", "safesearch", "pagination")
SUPPORTED_RESULT_TYPES: tuple[str, ...] = ("text", "answers", "corrections", "infoboxes", "media", "structured")
# Fine-grained media-type vocabulary for the dedicated image/video media
# contract (issue 188). Distinct from the coarse ``media`` result type above:
# ``media`` signals "may carry thumbnails/posters", while these tokens signal
# "performs image or video search and returns a structured media record".
SUPPORTED_MEDIA_TYPES: tuple[str, ...] = ("image", "video")
FAILURE_CLASS_TOKENS: tuple[str, ...] = (
    "ok",
    "rate_limited",
    "blocked",
    "error",
    "timeout",
    "auth_required",
    "unavailable",
)

# Coarse operator cost-class vocabulary (design §4.6, capability audit).
# Adapters declare exactly one value on the class; the catalog emits ``""``
# as ``null`` (explicit unknown — no fabricated estimates). Values:
#   "free"     — no API key and no paid tier required for the endpoints used
#   "freemium" — requires an API key but a usable free tier exists (may be
#                rate-limited or quota-bound)
#   "paid"     — requires paid API access (no meaningful free tier for the
#                endpoint used)
#   ""         — unknown / not audited (emitted as null)
COST_CLASSES: tuple[str, ...] = ("free", "freemium", "paid")


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
class MediaInfo:
    """Structured media record for image/video results (issue 188).

    The dedicated media result contract: safe URLs (the media file and its
    thumbnail), source attribution (the page the media was found on),
    dimensions (``width``/``height``) and ``duration`` (seconds) where the
    adapter observed them, and the closed ``media_type`` vocabulary.

    Absent/unknown fields stay ``None`` and are dropped at the serialization
    boundary so an adapter never fabricates a dimension or duration it did
    not actually receive from its source.
    """

    media_type: str  # "image" | "video"
    url: Optional[str] = None  # safe media file URL
    thumbnail: Optional[str] = None  # safe thumbnail URL
    source: Optional[str] = None  # source attribution (page URL)
    width: Optional[int] = None
    height: Optional[int] = None
    duration: Optional[float] = None  # seconds


def build_media(
    media_type: str,
    *,
    url: Optional[str] = None,
    thumbnail: Optional[str] = None,
    source: Optional[str] = None,
    width: Optional[int] = None,
    height: Optional[int] = None,
    duration: Optional[float] = None,
) -> Optional[MediaInfo]:
    """Build a validated :class:`MediaInfo`, sanitizing every URL.

    Returns ``None`` for a media type outside the closed
    :data:`SUPPORTED_MEDIA_TYPES` vocabulary so a typo can never fabricate a
    media record. URLs are passed through :func:`sanitize_url` so no
    credential-bearing query parameter can leak into a persisted/serialized
    media record.
    """
    if media_type not in SUPPORTED_MEDIA_TYPES:
        return None
    return MediaInfo(
        media_type=media_type,
        url=sanitize_url(url) if url else None,
        thumbnail=sanitize_url(thumbnail) if thumbnail else None,
        source=sanitize_url(source) if source else None,
        width=width,
        height=height,
        duration=duration,
    )


def media_to_dict(media: MediaInfo | None) -> dict[str, Any] | None:
    """Serialize a :class:`MediaInfo` to a JSON-safe dict, or ``None``.

    ``None`` values are dropped (never materialized as ``null``) so the
    canonical cache/snapshot form carries only the fields the adapter
    actually reported.
    """
    if media is None:
        return None
    data: dict[str, Any] = {"media_type": media.media_type}
    for key in ("url", "thumbnail", "source"):
        value = getattr(media, key)
        if value:
            data[key] = value
    for key in ("width", "height", "duration"):
        value = getattr(media, key)
        if value is not None:
            data[key] = value
    return data


def media_from_dict(value: Any) -> MediaInfo | None:
    """Rehydrate a :class:`MediaInfo` from a serialized value, or ``None``.

    Accepts a dict; anything else (missing, malformed) yields ``None`` so a
    broken media record never crashes the read path. A media type outside the
    closed vocabulary is rejected rather than rehydrated.
    """
    if not isinstance(value, dict):
        return None
    media_type = value.get("media_type")
    if media_type not in SUPPORTED_MEDIA_TYPES:
        return None
    return MediaInfo(
        media_type=media_type,
        url=value.get("url"),
        thumbnail=value.get("thumbnail"),
        source=value.get("source"),
        width=value.get("width"),
        height=value.get("height"),
        duration=value.get("duration"),
    )


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
    # Optional structured media record (image/video contract, issue 188).
    # ``None`` for text/non-media results. ``thumbnail``/``img_src`` remain
    # the legacy flat fields for backward compatibility.
    media: Optional[MediaInfo] = None
    # Optional versioned, JSON-safe domain payload (see slopsearx.payload).
    # None means the result has no domain-specific structured payload and the
    # common envelope is the complete representation.
    payload: Optional[dict[str, Any]] = None


class EngineStatus(enum.Enum):
    """Standardized engine health / error classification."""

    OK = "ok"
    RATE_LIMITED = "rate_limited"
    BLOCKED = "blocked"
    ERROR = "error"
    TIMEOUT = "timeout"
    UNAVAILABLE = "unavailable"


# Observed-health status vocabulary (issue 190). Every ``EngineStatus`` token
# plus ``unknown`` for engines with no recorded observation. This is the
# canonical set shared by the HTTP /health endpoint, the MCP status surface,
# and the capability catalog so all three agree on status semantics.
OBSERVED_STATUS_VOCAB: tuple[str, ...] = (
    "ok",
    "rate_limited",
    "blocked",
    "error",
    "timeout",
    "unavailable",
    "unknown",
)


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
    # True when the service fabricated this response (e.g. a deadline timeout
    # the adapter never returned from) instead of the adapter returning its
    # own classified outcome. Synthetic responses carry no *measured*
    # latency — ``latency_ms`` is a fabricated bound — so they must never
    # populate the observed-health latency field (issue 190).
    synthetic: bool = False


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

    # -- Declarative capability metadata (design §4.6) --------------------
    # These feed the live capability catalog and the filter-enforcement
    # report. Subclasses override them; the catalog provides stable
    # registry-derived defaults so an entry is always complete and honest
    # even when an adapter does not declare anything.
    #
    # Capability-audit convention (issue 185): every registered adapter
    # declares the capability surface it actually provides. ``sensitive``,
    # ``supported_result_types``, ``failure_classes``, and ``cost_class``
    # are audited per adapter. ``supported_filters`` stays at the base
    # default for every adapter: no adapter consumes the
    # ``language``/``time_range``/``safesearch``/``pagination`` parameter
    # bag today, so claiming support would be dishonest (the filter
    # enforcement report derives ``unsupported`` from exactly this).
    sensitive: bool = False  # reaching this engine requires the sensitive-engine grant
    supported_filters: dict[str, bool] = {}  # keys from SUPPORTED_FILTER_KEYS
    # Audited enforcement layer per filter (issue 187). Unlike
    # ``supported_filters`` (which only records whether the adapter consumes
    # the parameter bag), this records the layer that actually *enforces* the
    # filter: ``"upstream"`` (the upstream source applies it) or ``"local"``
    # (the service post-filters this adapter's results). An absent key means
    # the adapter does not enforce the filter. Defaults to ``{}``: no adapter
    # enforces any filter today, so the enforcement report stays honest.
    enforced_filters: dict[str, str] = {}  # filter name -> "upstream" | "local"
    supported_result_types: tuple[str, ...] = ("text",)  # subset of SUPPORTED_RESULT_TYPES
    # Dedicated image/video search capability (issue 188): which media types
    # this adapter can actively search and return structured media records
    # for. Empty means "no dedicated image/video search" — it does not claim
    # image/video routing even if the adapter may attach a thumbnail to text
    # results. Subset of SUPPORTED_MEDIA_TYPES.
    supported_media_types: tuple[str, ...] = ()
    failure_classes: tuple[str, ...] = (
        "rate_limited",
        "blocked",
        "error",
        "timeout",
        "auth_required",
        "unavailable",
    )  # subset of FAILURE_CLASS_TOKENS
    cost_class: str = ""  # one of COST_CLASSES; "" = unknown (emitted as null)

    # Circuit-breaker defaults (can be overridden per-instance via config or env vars)
    CIRCUIT_BREAKER_THRESHOLD: int = 5  # consecutive errors before circuit opens
    CIRCUIT_BREAKER_TIMEOUT: int = 300  # seconds circuit stays open

    def __init__(self, config: dict[str, Any] | None = None, rate_limiter: Any = None) -> None:
        self.config = config or {}
        self.rate_limiter = rate_limiter  # injected by server at startup
        # Use the adapter's declared categories unless config replaces them.
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

        # Passive observed health (issue 190). Updated by the search service
        # after each *dispatched* outcome with the classified status and
        # aggregate latency/result count — never the query or raw content.
        self.last_observed_status: str | None = None
        self.last_observed_at: float | None = None
        self.last_observed_latency_ms: float | None = None
        self.last_observed_result_count: int | None = None

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
        """Lightweight probe — does NOT hit external APIs.

        Returns OK if the engine is configured (has an API key if one
        is needed).  Actual endpoint health is verified at search time
        via the circuit breaker.  Every engine that gets a successful
        response is healthy; engines that time out or error out
        increment their failure counter and eventually trip the
        circuit breaker — no separate health-check API call needed.
        """
        # If the engine requires an API key, check it's configured
        api_key = self.config.get("api_key")
        if api_key is None or api_key == "":
            # Only flag as error if the engine type actually needs a key
            env_prefix = getattr(self, "env_prefix", None)
            if env_prefix:
                key_var = f"{env_prefix}_API_KEY"
                if key_var in self.config or self.config.get("api_key_required", False):
                    return EngineStatus.ERROR
        return EngineStatus.OK

    async def warmup(self) -> None:
        """Optional lifecycle hook — called at startup."""

    async def shutdown(self) -> None:
        """Optional lifecycle hook — called at graceful shutdown."""

    def _merge_categories(self) -> None:
        """Apply an optional category override to this adapter instance."""
        override = self.config.get("categories")
        self.categories = list(override) if override else list(type(self).categories)

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

    @property
    def circuit_open(self) -> bool:
        """Whether the circuit breaker is currently open (dispatches skipped)."""
        return self.circuit_open_until > time.time()

    def record_observation(
        self,
        status: EngineStatus,
        *,
        latency_ms: float | None = None,
        result_count: int | None = None,
    ) -> None:
        """Record a redacted observed-health summary for one search outcome.

        Called by the search service after a *dispatched* outcome. Only the
        classified status and aggregate latency/result count are kept; query
        text and result content are never stored (issue 190).

        ``latency_ms`` must be the adapter-reported latency when one exists
        and ``None`` for service-synthesized outcomes (e.g. a deadline
        timeout the adapter never returned from) so a fabricated bound is
        never surfaced as an observed latency.
        """
        self.last_observed_status = status.value
        self.last_observed_at = time.time()
        self.last_observed_latency_ms = latency_ms
        self.last_observed_result_count = result_count


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
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
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
            instances[name] = cls(cfg)
    return instances
