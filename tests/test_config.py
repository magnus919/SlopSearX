"""Tests for the layered configuration model."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

import yaml

from slopsearx.config import (
    CacheConfig,
    Config,
    EngineEntry,
    RankingConfig,
    load_config,
)


class TestEngineEntry:
    def test_defaults(self) -> None:
        entry = EngineEntry(base_url="https://example.com/api")
        assert entry.enabled is True
        assert entry.timeout_ms == 5_000
        assert entry.max_results == 10
        assert entry.weight == 1.0

    def test_override_timeout(self) -> None:
        entry = EngineEntry(base_url="https://example.com", timeout_ms=15_000)
        assert entry.timeout_ms == 15_000


class TestConfigDataclass:
    def test_empty_config(self) -> None:
        config = Config()
        assert config.cache.ttl_seconds == 300
        assert config.ranking.strategy == "presence"
        assert config.default_engines == ["brave", "wikipedia"]

    def test_custom_config(self) -> None:
        config = Config(
            engines={
                "brave": EngineEntry(
                    base_url="https://api.brave.com/search",
                    timeout_ms=3_000,
                ),
            },
            cache=CacheConfig(ttl_seconds=600),
            ranking=RankingConfig(strategy="weighted_fusion"),
            default_engines=["brave"],
        )
        assert config.engines["brave"].timeout_ms == 3_000
        assert config.cache.ttl_seconds == 600
        assert config.ranking.strategy == "weighted_fusion"
        assert config.default_engines == ["brave"]


class TestLoadConfig:
    """Layered config loading — defaults → file → env overrides."""

    def test_defaults_only(self) -> None:
        """With no config file and no env overrides, should return built-in defaults."""
        config = load_config()
        assert "brave" in config.engines
        assert config.engines["brave"].base_url == "https://api.search.brave.com/res/v1/web/search"
        assert config.cache.ttl_seconds == 300

    def test_config_file_overrides_defaults(self) -> None:
        """A config file should override built-in defaults."""
        data = {
            "engines": {
                "brave": {"timeout_ms": 2_000, "max_results": 5},
            },
            "cache": {"ttl_seconds": 600},
        }
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
            yaml.dump(data, f)
            f.flush()
            config_path = Path(f.name)

        try:
            config = load_config(config_path)
            assert config.engines["brave"].timeout_ms == 2_000
            assert config.engines["brave"].max_results == 5
            # Should keep other defaults
            assert config.engines["brave"].base_url == "https://api.search.brave.com/res/v1/web/search"
            assert config.cache.ttl_seconds == 600
        finally:
            config_path.unlink()

    def test_config_file_adds_new_engine(self) -> None:
        """A config file can add an engine not in defaults."""
        data = {
            "engines": {
                "customapi": {
                    "base_url": "https://custom.api/search",
                    "type": "api",
                    "timeout_ms": 4_000,
                },
            },
        }
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
            yaml.dump(data, f)
            config_path = Path(f.name)

        try:
            config = load_config(config_path)
            assert "customapi" in config.engines
            assert config.engines["customapi"].base_url == "https://custom.api/search"
        finally:
            config_path.unlink()

    def test_env_var_overrides_config_file(self) -> None:
        """Env vars should override config file values."""
        data = {
            "engines": {
                "brave": {"timeout_ms": 2_000},
            },
        }
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
            yaml.dump(data, f)
            config_path = Path(f.name)

        os.environ["ENGINE_BRAVE_TIMEOUT_MS"] = "1"
        try:
            config = load_config(config_path)
            # Env var should win
            assert config.engines["brave"].timeout_ms == 1
        finally:
            config_path.unlink()
            del os.environ["ENGINE_BRAVE_TIMEOUT_MS"]

    def test_missing_config_file_is_not_error(self) -> None:
        """If config file doesn't exist, use defaults without error."""
        config = load_config("/tmp/nonexistent-slopsearx-config.yaml")
        assert "brave" in config.engines
        assert config.cache.ttl_seconds == 300

    def test_env_var_global_settings(self) -> None:
        """SEARCH_* env vars should override global config settings."""
        os.environ["SEARCH_CACHE_TTL_SECONDS"] = "900"
        os.environ["SEARCH_LOG_LEVEL"] = "DEBUG"
        os.environ["SEARCH_DEFAULT_ENGINES"] = "wikipedia,brave"
        try:
            config = load_config()
            assert config.cache.ttl_seconds == 900
            assert config.log_level == "DEBUG"
            assert config.default_engines == ["wikipedia", "brave"]
        finally:
            del os.environ["SEARCH_CACHE_TTL_SECONDS"]
            del os.environ["SEARCH_LOG_LEVEL"]
            del os.environ["SEARCH_DEFAULT_ENGINES"]

    def test_engine_api_key_from_env(self) -> None:
        """ENGINE_*_API_KEY env var should be picked up as api_key."""
        os.environ["ENGINE_BRAVE_API_KEY"] = "super-secret-key"
        try:
            config = load_config()
            # The config loader should set api_key from the env var
            assert config.engines["brave"].api_key == "super-secret-key"
        finally:
            del os.environ["ENGINE_BRAVE_API_KEY"]
