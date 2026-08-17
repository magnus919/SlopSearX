"""Tests for the capability catalog, intent profiles, and MCP policy."""

from __future__ import annotations

import engines  # noqa: F401 — triggers @register_engine to populate registry
from slopsearx.adapter import COST_CLASSES, EngineAdapter
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


class TestCatalogFeatureMatrix:
    """The declarative per-engine capability matrix (design §4.6, capability-catalog)."""

    def test_supported_filters_always_has_all_four_keys(self) -> None:
        catalog = _catalog()
        for cap in catalog.all():
            assert set(cap.supported_filters) == {"language", "time_range", "safesearch", "pagination"}
            assert all(isinstance(v, bool) for v in cap.supported_filters.values())

    def test_supported_result_types_and_failure_classes_in_vocab(self) -> None:
        catalog = _catalog()
        from slopsearx.adapter import FAILURE_CLASS_TOKENS, SUPPORTED_RESULT_TYPES

        for cap in catalog.all():
            assert cap.supported_result_types
            assert set(cap.supported_result_types) <= set(SUPPORTED_RESULT_TYPES)
            assert cap.failure_classes
            assert set(cap.failure_classes) <= set(FAILURE_CLASS_TOKENS)

    def test_sensitive_defaults_and_policy_override(self) -> None:
        catalog = _catalog()
        assert catalog.get("hibp").sensitive is True  # type: ignore[union-attr]
        assert catalog.get("dehashed").sensitive is True  # type: ignore[union-attr]
        assert catalog.get("wikipedia").sensitive is False  # type: ignore[union-attr]

        override = _catalog(sensitive_engines={"cve"})
        assert override.get("cve").sensitive is True  # type: ignore[union-attr]
        assert override.get("hibp").sensitive is False  # type: ignore[union-attr]

    def test_cost_class_declared_or_explicitly_unknown(self) -> None:
        """Every engine has an audited cost class; '' is the explicit unknown.

        ``last_known_status`` stays ``unknown`` — it is observed passively
        through search outcomes and is never fabricated from declarations.
        """
        catalog = _catalog()
        for cap in catalog.all():
            assert cap.cost_class in COST_CLASSES or cap.cost_class == ""
            assert cap.last_known_status == "unknown"
            assert cap.last_known_status_at is None

    def test_representative_families_report_distinct_capabilities(self) -> None:
        """Audited declarations make domain families visibly distinct (issue 185)."""
        catalog = _catalog()

        # General/web: Brave carries answers + media; Wikipedia adds
        # corrections and infoboxes on top of media thumbnails.
        brave = catalog.get("brave")
        assert brave is not None
        assert set(brave.supported_result_types) == {"text", "answers", "media"}
        assert brave.cost_class == "freemium"
        wikipedia = catalog.get("wikipedia")
        assert wikipedia is not None
        assert set(wikipedia.supported_result_types) == {"text", "corrections", "infoboxes", "media"}
        assert wikipedia.cost_class == "free"

        # Packages: free, text-only registries.
        for name in ("pypi", "npm", "crates", "rubygems", "dockerhub", "repology"):
            cap = catalog.get(name)
            assert cap is not None
            assert cap.cost_class == "free"
            assert cap.supported_result_types == ["text"]

        # Science: free scholarly indexes.
        for name in ("arxiv", "openalex", "semanticscholar", "pubmed", "uniprot"):
            cap = catalog.get(name)
            assert cap is not None
            assert cap.cost_class == "free"

        # Security: keyed engines are freemium; keyless ones are free.
        for name in ("shodan", "censys", "virustotal", "abuseipdb", "otx", "intelx", "vulncheck", "hibp"):
            assert catalog.get(name).cost_class == "freemium"  # type: ignore[union-attr]
        for name in ("nvd", "cve", "urlhaus", "epss", "crtsh", "mitreattack", "exploitdb"):
            assert catalog.get(name).cost_class == "free"  # type: ignore[union-attr]
        for name in ("shodan", "censys", "cve", "nvd"):
            assert catalog.get(name).supported_result_types == ["text"]  # type: ignore[union-attr]
        assert catalog.get("dehashed").cost_class == "paid"  # type: ignore[union-attr]

        # Media/entertainment: TMDB returns poster thumbnails and needs a key.
        tmdb = catalog.get("tmdb")
        assert tmdb is not None
        assert "media" in tmdb.supported_result_types
        assert tmdb.cost_class == "freemium"

        # Jobs: free, text-only ATS boards.
        for name in ("greenhouse", "ashby", "lever"):
            cap = catalog.get(name)
            assert cap is not None
            assert cap.cost_class == "free"
            assert "jobs" in cap.categories

    def test_declared_failure_classes_match_adapter_behavior(self) -> None:
        """Failure classes are trimmed to what each adapter can actually emit."""
        catalog = _catalog()
        # Single-class failures: these adapters classify every upstream error
        # as a generic ERROR (no 429/403/timeout handling in the code path).
        assert catalog.get("openalex").failure_classes == ["error"]  # type: ignore[union-attr]
        assert catalog.get("internetarchive").failure_classes == ["error"]  # type: ignore[union-attr]
        # Two-class failures.
        assert catalog.get("hackernews").failure_classes == ["error", "timeout"]  # type: ignore[union-attr]
        assert catalog.get("pypi").failure_classes == ["error", "timeout"]  # type: ignore[union-attr]
        assert catalog.get("stackexchange").failure_classes == ["rate_limited", "error"]  # type: ignore[union-attr]
        # Four-class failures: standard 429/403/timeout/error handling.
        shodan = catalog.get("shodan")
        assert shodan is not None
        assert set(shodan.failure_classes) == {"rate_limited", "blocked", "error", "timeout"}

    def test_disabled_engines_expose_the_same_declarations(self) -> None:
        """include_disabled surfaces the audited matrix for disabled engines too."""
        catalog = _catalog()
        internetarchive = catalog.get("internetarchive")
        assert internetarchive is not None
        assert internetarchive.enabled is False
        assert internetarchive.cost_class == "free"
        assert internetarchive.supported_result_types == ["text"]
        assert internetarchive.failure_classes == ["error"]
        assert internetarchive.supported_filters["safesearch"] is False

    def test_catalog_reflects_instance_declarations_consistently(self) -> None:
        """The catalog normalizes instance declarations like class declarations.

        Adapters are the test-injection seam (MCP ``state_factory``); the
        catalog must reflect what the runtime adapter declares, not prose.
        """

        class _FakeWiki(EngineAdapter):
            name = "wikipedia"
            display_name = "Wikipedia"
            supported_filters = {"time_range": True}
            supported_result_types = ("text", "infoboxes")
            failure_classes = ("error",)
            cost_class = "paid"

            async def search(self, query, params=None):  # pragma: no cover
                from slopsearx.adapter import AdapterResponse, EngineStatus

                return AdapterResponse(results=[], status=EngineStatus.OK)

        catalog = CapabilityCatalog(config=load_config(), adapters={"wikipedia": _FakeWiki()})
        cap = catalog.get("wikipedia")
        assert cap is not None
        assert cap.supported_filters["time_range"] is True
        assert cap.supported_filters["safesearch"] is False
        assert cap.supported_result_types == ["text", "infoboxes"]
        assert cap.failure_classes == ["error"]
        assert cap.cost_class == "paid"

    def test_declared_supported_filter_is_not_an_enforcement_claim(self) -> None:
        """Declaring a filter is a capability hint, never an enforcement claim.

        The enforcement report is resolved against the dispatched scope at
        search time (see ``TestEnforcementAgainstDispatchedScope``); the
        catalog only reports the declaration as a boolean per filter key and
        does not fabricate any ``enforced`` status.
        """
        catalog = _catalog()
        for cap in catalog.all():
            assert set(cap.supported_filters) == {"language", "time_range", "safesearch", "pagination"}
            assert all(isinstance(v, bool) for v in cap.supported_filters.values())
            # No adapter consumes any filter parameter today (audited), so no
            # entry may claim a filter it cannot enforce.
            assert not any(cap.supported_filters.values())


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
