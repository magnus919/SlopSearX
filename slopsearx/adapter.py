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
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Optional

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

    def __init__(self, config: dict[str, Any] | None = None, rate_limiter: Any = None) -> None:
        self.config = config or {}
        self.rate_limiter = rate_limiter  # injected by server at startup
        # Merge categories: self-declared default + config override/add/remove
        self._merge_categories()

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


class ScrapeAdapter(EngineAdapter, ABC):
    """Base class for scrape-based engines (DDG, Google).

    Scrape adapters send HTTP GET/POST requests with stealth headers
    and parse HTML responses — no headless browser required.
    """

    engine_type = "scrape"

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
