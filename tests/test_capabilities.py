"""Tests for the capability catalog, intent profiles, and MCP policy."""

from __future__ import annotations

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.capabilities import (
    AUTH_NONE,
    AUTH_REQUIRED,
    DEFAULT_SENSITIVE_ENGINES,
    INTENT_PROFILES,
    CapabilityCatalog,
    MCPPolicy,
    load_mcp_policy,
    resolve_intent,
    validate_intent_profiles,
)
from slopsearx.config import EngineEntry, load_config


def _catalog(**overrides) -> CapabilityCatalog:
    config = overrides.get("config") or load_config()
    return CapabilityCatalog(config=config, **{k: v for k, v in overrides.items() if k != "config"})


class TestCatalogBasics:
    def test_includes_all_registered_engines(self) -> None:
        catalog = _catalog()
        # Registry has 51 adapters; catalog must match the live registry,
        # not prose counts in the README.
        assert len(catalog.all()) >= 51
        for cap in catalog.all():
            assert cap.name
            assert cap.display_name
            assert cap.engine_type in ("api", "scrape", "structured")

    def test_includes_disabled_engines_with_flag(self) -> None:
        catalog = _catalog()
        internetarchive = catalog.get("internetarchive")
        assert internetarchive is not None
        assert internetarchive.enabled is False  # opt-in engine

    def test_auth_classes(self) -> None:
        catalog = _catalog()
        assert catalog.get("brave").auth_class == AUTH_REQUIRED  # type: ignore[union-attr]
        assert catalog.get("shodan").auth_class == AUTH_REQUIRED  # type: ignore[union-attr]
        assert catalog.get("wikipedia").auth_class == AUTH_NONE  # type: ignore[union-attr]
        assert catalog.get("reddit").auth_class == AUTH_NONE  # type: ignore[union-attr]

    def test_auth_configured_reflects_key_presence(self) -> None:
        config = load_config()
        config.engines["brave"] = EngineEntry(api_key="secret-key-123")
        config.engines["wikipedia"] = EngineEntry()
        catalog = _catalog(config=config)

        assert catalog.get("brave").auth_configured is True  # type: ignore[union-attr]
        assert catalog.get("wikipedia").auth_configured is False  # type: ignore[union-attr]

    def test_catalog_never_leaks_secrets(self) -> None:
        config = load_config()
        config.engines["brave"] = EngineEntry(api_key="super-secret-value")
        catalog = _catalog(config=config)
        import dataclasses

        for cap in catalog.all():
            as_dict = dataclasses.asdict(cap)
            blob = str(as_dict)
            assert "super-secret-value" not in blob
            assert "api_key" not in blob

    def test_subcategories(self) -> None:
        catalog = _catalog()
        github = catalog.get("github")
        assert github is not None
        assert "github:code" in github.subcategories

    def test_caveats(self) -> None:
        catalog = _catalog()
        github = catalog.get("github")
        assert github is not None
        assert any("page 1" in caveat for caveat in github.caveats)

    def test_scope_hints(self) -> None:
        catalog = _catalog()
        assert "unscoped" in catalog.get("wikipedia").scope_hints  # type: ignore[union-attr]
        assert "unscoped" not in catalog.get("shodan").scope_hints  # type: ignore[union-attr]

    def test_engines_for_categories(self) -> None:
        catalog = _catalog()
        security = set(catalog.engines_for_categories(["security"]))
        assert "nvd" in security
        assert "shodan" in security
        assert "wikipedia" not in security  # not a security engine

    def test_families(self) -> None:
        catalog = _catalog()
        families = catalog.families()
        assert "security" in families
        assert "packages" in families
        assert all(isinstance(v, list) and v for v in families.values())


class TestIntentProfiles:
    def test_code_profile_known_engines(self) -> None:
        catalog = _catalog()
        engines_list, warnings = resolve_intent("code", catalog)
        assert warnings == []
        assert "github" in engines_list
        assert "pypi" in engines_list
        # Only registry-known engines are returned
        assert all(name in catalog.known_names() for name in engines_list)

    def test_unknown_intent_warns(self) -> None:
        catalog = _catalog()
        engines_list, warnings = resolve_intent("bogus", catalog)
        assert engines_list == []
        assert any("unknown intent" in w for w in warnings)

    def test_security_profile_is_sensitive(self) -> None:
        assert INTENT_PROFILES["security"].sensitive is True
        assert INTENT_PROFILES["web"].sensitive is False

    def test_all_profiles_validate_against_registry(self) -> None:
        catalog = _catalog()
        problems = validate_intent_profiles(catalog)
        assert problems == [], f"intent profile problems: {problems}"

    def test_category_profiles_resolve_to_enabled_engines(self) -> None:
        catalog = _catalog()
        for intent in ("web", "news", "science", "reference", "finance", "packages", "legal", "geography"):
            engines_list, warnings = resolve_intent(intent, catalog)
            assert warnings == [], f"{intent}: {warnings}"
            assert engines_list, f"intent '{intent}' resolved to no engines"
            assert all(catalog.get(name).enabled for name in engines_list)  # type: ignore[union-attr]


class TestMCPPolicy:
    def test_defaults_are_secure(self) -> None:
        policy = load_mcp_policy(config_path=None)
        assert policy.tool_enabled("jobs") is False
        assert policy.tool_enabled("security") is False
        assert policy.tool_enabled("science") is False
        assert policy.tool_enabled("research") is False
        assert policy.sensitive_engines == set(DEFAULT_SENSITIVE_ENGINES)
        assert policy.max_results == 50
        assert policy.auth_token == ""

    def test_yaml_section_applied(self, tmp_path) -> None:
        config_file = tmp_path / "config.yaml"
        config_file.write_text(
            """
mcp:
  enabled_tools:
    jobs: true
    security: true
  sensitive_engines: [hibp]
  max_results: 25
  job_max_queries: 5
  auth_token: "s3cret"
"""
        )
        policy = load_mcp_policy(config_path=config_file)
        assert policy.tool_enabled("jobs") is True
        assert policy.tool_enabled("security") is True
        assert policy.tool_enabled("science") is False
        assert policy.sensitive_engines == {"hibp"}
        assert policy.max_results == 25
        assert policy.job_max_queries == 5
        assert policy.auth_token == "s3cret"

    def test_env_overrides_beat_yaml(self, tmp_path, monkeypatch) -> None:
        config_file = tmp_path / "config.yaml"
        config_file.write_text("mcp:\n  enabled_tools:\n    jobs: true\n")
        monkeypatch.setenv("MCP_GRANT_JOBS", "0")
        monkeypatch.setenv("MCP_MAX_RESULTS", "100")
        policy = load_mcp_policy(config_path=config_file)
        assert policy.tool_enabled("jobs") is False  # env wins
        assert policy.max_results == 100

    def test_invalid_env_values_ignored(self, monkeypatch) -> None:
        monkeypatch.setenv("MCP_MAX_RESULTS", "not-a-number")
        policy = load_mcp_policy(config_path=None)
        assert policy.max_results == 50

    def test_validate_catches_unknown_engines(self) -> None:
        policy = MCPPolicy()
        policy.sensitive_engines = {"not-a-real-engine"}
        catalog = _catalog()
        problems = policy.validate(catalog)
        assert any("not-a-real-engine" in p for p in problems)

    def test_validate_clean_for_defaults(self) -> None:
        policy = load_mcp_policy(config_path=None)
        catalog = _catalog()
        assert policy.validate(catalog) == []
