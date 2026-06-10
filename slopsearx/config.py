"""
SlopSearX — Layered Configuration Model.

Three-layer loading: built-in defaults → optional YAML file → env var overrides.
Env vars always win over file values for the same key.
"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Optional

import yaml

# ---------------------------------------------------------------------------
# Config dataclass
# ---------------------------------------------------------------------------

CONFIG_FILE_PATH = "/etc/slopsearx/config.yaml"


@dataclass
class CacheConfig:
    ttl_seconds: int = 300
    max_result_sets: int = 10_000
    revalidate_on_hit: bool = False


@dataclass
class RankingConfig:
    strategy: str = "presence"  # "presence" | "weighted_fusion" | "learning_to_rank"


@dataclass
class EngineEntry:
    enabled: bool = True
    base_url: str = ""
    type: str = "api"  # "api" | "scrape" | "structured"
    timeout_ms: int = 5_000
    max_results: int = 10
    rate_limit: Optional[float] = None  # requests per second
    weight: float = 1.0
    api_key: Optional[str] = None
    # category support
    categories: Optional[list[str]] = None  # full override
    categories_add: Optional[list[str]] = None  # append
    categories_remove: Optional[list[str]] = None  # suppress
    # scrape-specific fields
    proxy_pool: Optional[str] = None
    scrape_proxy_url: Optional[str] = None

    def __post_init__(self) -> None:
        if self.api_key is None:
            self.api_key = os.environ.get(f"ENGINE_{self.base_url.upper()}_API_KEY")


@dataclass
class Config:
    """Top-level configuration object."""

    engines: dict[str, EngineEntry] = field(default_factory=dict)
    cache: CacheConfig = field(default_factory=CacheConfig)
    ranking: RankingConfig = field(default_factory=RankingConfig)

    # Global settings
    default_engines: list[str] = field(default_factory=lambda: ["brave", "wikipedia"])
    log_level: str = "INFO"


# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------

_DEFAULT_ENGINES: dict[str, dict[str, Any]] = {
    "arxiv": {
        "base_url": "http://export.arxiv.org/api/query",
        "type": "api",
        "timeout_ms": 10_000,
        "max_results": 5,
        "rate_limit": 0.33,  # 1 req per 3 seconds (arXiv ToS)
        "weight": 0.8,
    },
    "brave": {
        "base_url": "https://api.search.brave.com/res/v1/web/search",
        "type": "api",
        "timeout_ms": 5_000,
        "max_results": 10,
        "rate_limit": 15,
        "weight": 1.0,
    },
    "duckduckgo": {
        "base_url": "https://html.duckduckgo.com/html/",
        "type": "scrape",
        "timeout_ms": 10_000,
        "max_results": 10,
        "weight": 0.6,
    },
    "github": {
        "base_url": "https://api.github.com",
        "type": "api",
        "timeout_ms": 5_000,
        "max_results": 5,
        "rate_limit": 0.5,  # 30 req/min with token
        "weight": 0.8,
    },
    "google": {
        "base_url": "https://www.google.com/search",
        "type": "scrape",
        "timeout_ms": 10_000,
        "max_results": 10,
        "weight": 0.5,
    },
    "hackernews": {
        "base_url": "https://hn.algolia.com/api/v1/search",
        "type": "api",
        "timeout_ms": 3_000,
        "max_results": 5,
        "rate_limit": 10,
        "weight": 0.7,
    },
    "huggingface": {
        "base_url": "https://huggingface.co/api",
        "type": "api",
        "timeout_ms": 5_000,
        "max_results": 5,
        "rate_limit": 1,
        "weight": 0.7,
    },
    "semanticscholar": {
        "base_url": "https://api.semanticscholar.org/graph/v1/paper/search",
        "type": "api",
        "timeout_ms": 5_000,
        "max_results": 5,
        "rate_limit": 1,  # 1 req/s without key, 10 req/s with
        "weight": 0.8,
    },
    "stackexchange": {
        "base_url": "https://api.stackexchange.com/2.3",
        "type": "api",
        "timeout_ms": 5_000,
        "max_results": 10,
        "rate_limit": 30,  # 30 req/s with key
        "weight": 0.9,
    },
    "openalex": {
        "base_url": "https://api.openalex.org",
        "type": "api",
        "timeout_ms": 5_000,
        "max_results": 10,
        "rate_limit": 10,  # 100K/day polite usage
        "weight": 0.8,
    },
    "internetarchive": {
        "base_url": "https://archive.org",
        "type": "api",
        "timeout_ms": 10_000,
        "max_results": 10,
        "rate_limit": 5,  # conservative, IA can be slow
        "weight": 0.6,
        "enabled": False,  # opt-in only — not in default search path
    },
    "reddit": {
        "base_url": "https://www.reddit.com",
        "type": "api",
        "timeout_ms": 5_000,
        "max_results": 10,
        "rate_limit": 1,  # conservative: 1 req/s (~60 req/min)
        "weight": 0.7,
    },
    "wikipedia": {
        "base_url": "https://en.wikipedia.org/w/api.php",
        "type": "api",
        "timeout_ms": 3_000,
        "max_results": 3,
        "rate_limit": 200,
        "weight": 0.9,
    },
}

_DEFAULT_CACHE = {"ttl_seconds": 300, "max_result_sets": 10_000, "revalidate_on_hit": False}
_DEFAULT_RANKING = {"strategy": "presence"}


# ---------------------------------------------------------------------------
# Loader
# ---------------------------------------------------------------------------


def _engine_name_to_env_prefix(name: str) -> str:
    """Convert engine name to environment variable prefix.

    >>> _engine_name_to_env_prefix("duckduckgo")
    'ENGINE_DUCKDUCKGO_'
    """
    return f"ENGINE_{name.upper()}_"


def _load_env_overrides() -> dict[str, Any]:
    """Read all ``SEARCH_*`` and ``ENGINE_*`` env vars and return a flat dict."""
    overrides: dict[str, Any] = {}
    for key, value in os.environ.items():
        if key.startswith("SEARCH_"):
            overrides[key.lower()] = value
        elif key.startswith("ENGINE_"):
            # ENGINE_BRAVE_API_KEY → brave.api_key
            parts = key.split("_", 2)
            if len(parts) < 3:
                continue
            engine_name = parts[1].lower()
            setting = parts[2].lower()
            overrides[f"engines.{engine_name}.{setting}"] = value
    return overrides


def _dict_to_config(data: dict[str, Any]) -> Config:
    """Convert a nested dict to a Config dataclass."""
    engines = {}
    for name, eng_data in data.get("engines", {}).items():
        if isinstance(eng_data, dict):
            eng_data.setdefault("api_key", os.environ.get(f"ENGINE_{name.upper()}_API_KEY"))
            engines[name] = EngineEntry(**eng_data)

    cache_data = data.get("cache", {})
    cache = CacheConfig(**{**cache_data, **{k: v for k, v in _DEFAULT_CACHE.items() if k not in cache_data}})

    ranking_data = data.get("ranking", {})
    ranking = RankingConfig(**{**ranking_data, **{k: v for k, v in _DEFAULT_RANKING.items() if k not in ranking_data}})

    return Config(
        engines=engines or {name: EngineEntry(**cfg) for name, cfg in _DEFAULT_ENGINES.items()},
        cache=cache,
        ranking=ranking,
        default_engines=data.get("default_engines", ["brave", "wikipedia"]),
        log_level=data.get("log_level", "INFO"),
    )


def _apply_env_overrides(config: Config, overrides: dict[str, str]) -> Config:
    """Apply flat env-var overrides to a resolved Config object."""
    for key, value in overrides.items():
        if key.startswith("engines."):
            parts = key.split(".")
            if len(parts) >= 3:
                engine_name = parts[1]
                setting = ".".join(parts[2:])
                if engine_name in config.engines:
                    # List coercion for categories fields
                    if setting in ("categories", "categories_add", "categories_remove"):
                        typed_value = [c.strip() for c in value.split(",") if c.strip()] if value else []
                    else:
                        typed_value = _coerce_type(value, type(getattr(config.engines[engine_name], setting, str)))
                    setattr(config.engines[engine_name], setting, typed_value)
        elif key == "search_cache_ttl_seconds" and hasattr(config.cache, "ttl_seconds"):
            config.cache.ttl_seconds = int(value)
        elif key == "search_log_level":
            config.log_level = value.upper()
        elif key == "search_default_engines":
            config.default_engines = [e.strip() for e in value.split(",")]
    return config


def _coerce_type(value: str, target_type: type) -> Any:
    """Cast a string to the target type for env-var override."""
    if target_type is bool:
        return value.lower() in ("true", "1", "yes")
    if target_type is int:
        return int(value)
    if target_type is float:
        return float(value)
    return value


def _merge_engine_configs(
    file_config: dict[str, EngineEntry] | None,
    defaults: dict[str, dict[str, Any]],
) -> dict[str, EngineEntry]:
    """Merge file-based engine config over defaults."""
    result: dict[str, EngineEntry] = {}
    if file_config:
        for name, entry in file_config.items():
            result[name] = entry
    for name, cfg_dict in defaults.items():
        if name not in result:
            result[name] = EngineEntry(**cfg_dict)
    return result


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def load_config(
    config_path: str | Path | None = None,
) -> Config:
    """Load the layered configuration.

    Priority (highest wins):
      1. Env vars (``ENGINE_*`` / ``SEARCH_*``)
      2. Config file (YAML at ``config_path`` or fallback path)
      3. Built-in defaults (hardcoded in this module)

    Args:
        config_path: Path to the YAML config file. If ``None``,
            falls back to ``/etc/slopsearx/config.yaml``.

    Returns:
        A fully resolved ``Config`` dataclass.
    """
    # 1. Start with built-in defaults
    engines = {name: EngineEntry(**cfg) for name, cfg in _DEFAULT_ENGINES.items()}
    config = Config(
        engines=engines,
        cache=CacheConfig(),
        ranking=RankingConfig(),
    )

    # 2. Layer: config file
    path = Path(config_path) if config_path else Path(CONFIG_FILE_PATH)
    file_data: dict[str, Any] = {}
    if path.exists():
        with open(path) as f:
            file_data = yaml.safe_load(f) or {}

    if file_data:
        # Rebuild engines from file + defaults
        file_engines_raw = file_data.get("engines", {})
        merged_engines: dict[str, EngineEntry] = {}
        for name, cfg_dict in _DEFAULT_ENGINES.items():
            merged_engines[name] = EngineEntry(**{**cfg_dict, **file_engines_raw.get(name, {})})
        for name, raw in file_engines_raw.items():
            if name not in merged_engines:
                merged_engines[name] = EngineEntry(**raw)
        config.engines = merged_engines

        if "cache" in file_data:
            for k, v in file_data["cache"].items():
                setattr(config.cache, k, v)
        if "ranking" in file_data:
            for k, v in file_data["ranking"].items():
                setattr(config.ranking, k, v)
        config.default_engines = file_data.get("default_engines", config.default_engines)
        config.log_level = file_data.get("log_level", config.log_level)

    # 3. Layer: env vars
    overrides = _load_env_overrides()
    config = _apply_env_overrides(config, overrides)

    return config
