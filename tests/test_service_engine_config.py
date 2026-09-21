"""Tests for the service-to-adapter engine configuration boundary."""

from __future__ import annotations

from typing import Any

import pytest

import engines  # noqa: F401 — trigger @register_engine
from slopsearx import service as service_module
from slopsearx.adapter import discover_engines
from slopsearx.config import Config, EngineEntry
from slopsearx.service import _normalize_engine_configs


def test_blank_base_url_is_omitted_without_mutating_other_config() -> None:
    raw = {
        "enabled": True,
        "base_url": "  \t",
        "timeout_ms": 1234,
        "api_key": "configured",
        "categories": ["reference"],
        "_feature_brave_category_routing": True,
    }

    normalized = _normalize_engine_configs({"example": raw})

    assert "base_url" not in normalized["example"]
    assert normalized["example"]["timeout_ms"] == 1234
    assert normalized["example"]["api_key"] == "configured"
    assert normalized["example"]["categories"] == ["reference"]
    assert normalized["example"]["_feature_brave_category_routing"] is True
    assert raw["base_url"] == "  \t"


def test_none_base_url_is_treated_as_unset() -> None:
    normalized = _normalize_engine_configs({"example": {"base_url": None, "api_key": "configured"}})

    assert normalized == {"example": {"api_key": "configured"}}


def test_nonempty_base_url_override_is_preserved_exactly() -> None:
    custom_url = "https://operator.example/search"

    normalized = _normalize_engine_configs({"example": {"base_url": custom_url, "timeout_ms": 900}})

    assert normalized["example"] == {"base_url": custom_url, "timeout_ms": 900}


@pytest.mark.asyncio
async def test_build_context_omits_blank_configured_base_url(monkeypatch: pytest.MonkeyPatch) -> None:
    """The real startup path must pass provider defaults to discovered adapters."""
    config = Config(
        engines={name: EngineEntry(base_url="", api_key="configured") for name in ("github", "pubmed", "uniprot")}
    )
    captured: dict[str, dict[str, Any]] = {}

    def discover_with_capture(engine_configs: dict[str, dict[str, Any]]) -> dict[str, Any]:
        captured.update(engine_configs)
        return discover_engines(engine_configs)

    async def skip_warmup(name: str, adapter: Any) -> None:
        del name, adapter

    monkeypatch.setattr(service_module, "load_config", lambda: config)
    monkeypatch.setattr(service_module, "discover_engines", discover_with_capture)
    monkeypatch.setattr(service_module, "_warmup_engine", skip_warmup)

    context = await service_module.build_context()
    try:
        for name in ("github", "pubmed", "uniprot"):
            assert "base_url" not in captured[name]
            assert "base_url" not in context.active_engines[name].config
    finally:
        await service_module.destroy_context(context)
