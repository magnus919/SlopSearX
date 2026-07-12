"""Tests for the feature flag system in slopsearx/config.py."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

import yaml
from pytest import MonkeyPatch

from slopsearx.config import FeatureFlags, _load_env_overrides, load_config


class TestFeatureFlagsDataclass:
    def test_default_all_flags_disabled(self) -> None:
        ff = FeatureFlags()
        assert ff.is_enabled("nonexistent") is False
        assert ff.is_enabled("ai_dispatch") is False

    def test_explicit_flag_enabled(self) -> None:
        ff = FeatureFlags(flags={"ai_dispatch": True})
        assert ff.is_enabled("ai_dispatch") is True
        assert ff.is_enabled("other") is False

    def test_flag_explicitly_disabled(self) -> None:
        ff = FeatureFlags(flags={"ai_dispatch": False})
        assert ff.is_enabled("ai_dispatch") is False


class TestFeatureFlagsFromYaml:
    def test_yaml_features_loaded(self) -> None:
        yaml_content: dict[str, object] = {
            "features": {"ai_dispatch": True, "experimental_ranking": False},
        }
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
            yaml.dump(yaml_content, f)
            temp_path = f.name

        try:
            config = load_config(config_path=temp_path)
            assert config.feature_flags.is_enabled("ai_dispatch") is True
            assert config.feature_flags.is_enabled("experimental_ranking") is False
            assert config.feature_flags.is_enabled("nonexistent") is False
        finally:
            Path(temp_path).unlink()

    def test_yaml_without_features_section(self) -> None:
        yaml_content: dict[str, object] = {"cache": {"ttl_seconds": 600}}
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
            yaml.dump(yaml_content, f)
            temp_path = f.name

        try:
            config = load_config(config_path=temp_path)
            assert config.feature_flags.is_enabled("anything") is False
        finally:
            Path(temp_path).unlink()


class TestFeatureFlagsFromEnv:
    def test_feature_env_vars_are_collected_as_overrides(self, monkeypatch: MonkeyPatch) -> None:
        monkeypatch.setenv("FEATURE_AI_DISPATCH", "true")

        overrides = _load_env_overrides()

        assert overrides["features.ai_dispatch"] == "true"

    def test_env_overrides_yaml(self) -> None:
        yaml_content: dict[str, object] = {
            "features": {"ai_dispatch": False},
        }
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
            yaml.dump(yaml_content, f)
            temp_path = f.name

        try:
            os.environ["FEATURE_AI_DISPATCH"] = "true"
            config = load_config(config_path=temp_path)
            assert config.feature_flags.is_enabled("ai_dispatch") is True
        finally:
            Path(temp_path).unlink()
            del os.environ["FEATURE_AI_DISPATCH"]

    def test_env_set_to_false(self) -> None:
        os.environ["FEATURE_AI_DISPATCH"] = "false"
        try:
            config = load_config()
            assert config.feature_flags.is_enabled("ai_dispatch") is False
        finally:
            del os.environ["FEATURE_AI_DISPATCH"]

    def test_env_value_1_is_true(self) -> None:
        os.environ["FEATURE_AI_DISPATCH"] = "1"
        try:
            config = load_config()
            assert config.feature_flags.is_enabled("ai_dispatch") is True
        finally:
            del os.environ["FEATURE_AI_DISPATCH"]

    def test_env_value_0_is_false(self) -> None:
        yaml_content: dict[str, object] = {
            "features": {"ai_dispatch": True},
        }
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
            yaml.dump(yaml_content, f)
            temp_path = f.name

        try:
            os.environ["FEATURE_AI_DISPATCH"] = "0"
            config = load_config(config_path=temp_path)
            assert config.feature_flags.is_enabled("ai_dispatch") is False
        finally:
            Path(temp_path).unlink()
            del os.environ["FEATURE_AI_DISPATCH"]

    def test_env_value_yes_is_true(self) -> None:
        os.environ["FEATURE_AI_DISPATCH"] = "yes"
        try:
            config = load_config()
            assert config.feature_flags.is_enabled("ai_dispatch") is True
        finally:
            del os.environ["FEATURE_AI_DISPATCH"]

    def test_invalid_env_value_treated_as_false(self) -> None:
        os.environ["FEATURE_AI_DISPATCH"] = "maybe"
        try:
            config = load_config()
            assert config.feature_flags.is_enabled("ai_dispatch") is False
        finally:
            del os.environ["FEATURE_AI_DISPATCH"]

    def test_multiple_env_flags(self) -> None:
        os.environ["FEATURE_AI_DISPATCH"] = "true"
        os.environ["FEATURE_EXPERIMENTAL"] = "1"
        os.environ["FEATURE_LEGACY"] = "false"
        try:
            config = load_config()
            assert config.feature_flags.is_enabled("ai_dispatch") is True
            assert config.feature_flags.is_enabled("experimental") is True
            assert config.feature_flags.is_enabled("legacy") is False
        finally:
            del os.environ["FEATURE_AI_DISPATCH"]
            del os.environ["FEATURE_EXPERIMENTAL"]
            del os.environ["FEATURE_LEGACY"]

    def test_env_flag_caseless(self) -> None:
        os.environ["FEATURE_AI_DISPATCH"] = "TRUE"
        try:
            config = load_config()
            assert config.feature_flags.is_enabled("ai_dispatch") is True
        finally:
            del os.environ["FEATURE_AI_DISPATCH"]
