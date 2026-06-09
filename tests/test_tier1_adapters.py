"""Tests for Tier 1 adapters — Stack Exchange, OpenAlex, Internet Archive."""

from __future__ import annotations

import engines  # noqa: F401

# --- Stack Exchange ---


class TestStackExchangeAdapter:
    """StackExchange adapter."""

    def test_site_from_categories_defaults_to_stackoverflow(self) -> None:
        """No sub-category → stackoverflow."""
        from engines.stackexchange import StackExchangeAdapter

        assert StackExchangeAdapter._site_from_categories([]) == "stackoverflow"
        assert StackExchangeAdapter._site_from_categories(["general"]) == "stackoverflow"

    def test_site_from_categories_serverfault(self) -> None:
        """stackexchange:serverfault → serverfault."""
        from engines.stackexchange import StackExchangeAdapter

        assert StackExchangeAdapter._site_from_categories(["stackexchange:serverfault"]) == "serverfault"

    def test_site_from_categories_code(self) -> None:
        """stackexchange:code → stackoverflow."""
        from engines.stackexchange import StackExchangeAdapter

        assert StackExchangeAdapter._site_from_categories(["stackexchange:code"]) == "stackoverflow"

    def test_site_code_wins_over_serverfault_in_order(self) -> None:
        """First matching category wins."""
        from engines.stackexchange import StackExchangeAdapter

        result = StackExchangeAdapter._site_from_categories(["stackexchange:code", "stackexchange:serverfault"])
        assert result == "stackoverflow"  # code appears first

    def test_adapter_registered(self) -> None:
        """Adapter is in registry."""
        from slopsearx.adapter import list_engines

        assert "stackexchange" in list_engines()

    def test_adapter_categories(self) -> None:
        """Has correct category tags."""
        from slopsearx.adapter import list_engines

        cls = list_engines()["stackexchange"]
        cats = cls.categories
        assert "general" in cats
        assert "stackexchange:code" in cats
        assert "stackexchange:serverfault" in cats


# --- OpenAlex ---


class TestOpenAlexAdapter:
    """OpenAlex adapter."""

    def test_reconstruct_abstract_empty(self) -> None:
        """Empty/none inverted index → empty string."""
        from engines.openalex import _reconstruct_abstract

        assert _reconstruct_abstract(None) == ""
        assert _reconstruct_abstract({}) == ""

    def test_reconstruct_abstract_simple(self) -> None:
        """Simple inverted index reconstruction."""
        from engines.openalex import _reconstruct_abstract

        inverted = {"hello": [0], "world": [1]}
        result = _reconstruct_abstract(inverted)
        assert result == "hello world"

    def test_reconstruct_abstract_multi_position(self) -> None:
        """Word appearing at multiple positions."""
        from engines.openalex import _reconstruct_abstract

        inverted = {"the": [0, 3], "cat": [1], "sat": [2]}
        result = _reconstruct_abstract(inverted)
        assert result == "the cat sat the"

    def test_adapter_registered(self) -> None:
        """Adapter is in registry."""
        from slopsearx.adapter import list_engines

        assert "openalex" in list_engines()

    def test_adapter_categories(self) -> None:
        """Has correct category tags."""
        from slopsearx.adapter import list_engines

        cls = list_engines()["openalex"]
        cats = cls.categories
        assert "general" in cats
        assert "science" in cats
        assert "reference" in cats


# --- Internet Archive ---


class TestInternetArchiveAdapter:
    """Internet Archive adapter."""

    def test_adapter_registered(self) -> None:
        """Adapter is in registry."""
        from slopsearx.adapter import list_engines

        assert "internetarchive" in list_engines()

    def test_adapter_categories_excludes_general(self) -> None:
        """IA deliberately excludes general category."""
        from slopsearx.adapter import list_engines

        cls = list_engines()["internetarchive"]
        cats = cls.categories
        assert "general" not in cats
        assert "web:archive" in cats
        assert "historical" in cats
        assert "reference" in cats

    def test_adapter_default_disabled(self) -> None:
        """IA is disabled by default in config."""
        from slopsearx.config import _DEFAULT_ENGINES

        cfg = _DEFAULT_ENGINES["internetarchive"]
        assert cfg.get("enabled") is False
