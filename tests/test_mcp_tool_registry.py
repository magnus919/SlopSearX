"""Invariant tests for the declarative MCP tool registry."""

from __future__ import annotations

import inspect
from dataclasses import replace
from pathlib import Path

import pytest

from slopsearx.capabilities import MCPPolicy
from slopsearx.mcp import dependency_tools, lineage_tools, receipt_tools, staged_tools, tools
from slopsearx.mcp.server import create_server
from slopsearx.mcp.tool_registry import (
    TOOL_DEFINITIONS,
    SensitiveEngineBehavior,
    StateRequirement,
    tool_names,
    validate_tool_registry,
)

GOLDEN_TOOL_NAMES = (
    "slopsearx_search",
    "slopsearx_search_targeted",
    "slopsearx_search_jobs",
    "slopsearx_search_security",
    "slopsearx_search_science",
    "slopsearx_list_capabilities",
    "slopsearx_explain_search_scope",
    "slopsearx_get_service_status",
    "slopsearx_read_results",
    "slopsearx_read_result",
    "slopsearx_read_entities",
    "slopsearx_start_research",
    "slopsearx_get_job",
    "slopsearx_cancel_job",
    "slopsearx_retry_research",
    "slopsearx_extend_research",
    "slopsearx_update_research",
    "slopsearx_create_saved_search",
    "slopsearx_get_saved_search",
    "slopsearx_update_saved_search",
    "slopsearx_pause_saved_search",
    "slopsearx_delete_saved_search",
    "slopsearx_read_saved_search_reports",
    "slopsearx_read_saved_search_events",
    "slopsearx_ack_saved_search_events",
    "slopsearx_submit_retrieval_receipt",
    "slopsearx_read_retrieval_receipts",
    "slopsearx_export_research_manifest",
    "slopsearx_preview_staged_search",
    "slopsearx_search_staged",
    "slopsearx_get_staged_search",
    "slopsearx_retry_staged_search",
    "slopsearx_start_dependency_dossier",
    "slopsearx_get_dependency_dossier",
    "slopsearx_get_artifact_lineage",
)


def _exported_tool_names() -> set[str]:
    exported: set[str] = set()
    for module in (tools, receipt_tools, staged_tools, dependency_tools, lineage_tools):
        exported.update(
            name
            for name, value in vars(module).items()
            if name.startswith("slopsearx_") and inspect.iscoroutinefunction(value)
        )
    return exported


def test_golden_inventory_detects_additions_removals_and_renames() -> None:
    assert tool_names() == GOLDEN_TOOL_NAMES


def test_every_exported_tool_appears_exactly_once() -> None:
    names = tool_names()
    assert len(names) == len(set(names))
    assert set(names) == _exported_tool_names()


def test_every_definition_has_auditable_metadata() -> None:
    known_grants = set(MCPPolicy().enabled_tools)
    for definition in TOOL_DEFINITIONS:
        assert definition.name == definition.callable.__name__
        assert definition.owner_module == definition.callable.__module__
        assert definition.contract.startswith("slopsearx.")
        assert definition.contract_version == "1.0"
        assert set(definition.required_grants) <= known_grants
        assert isinstance(definition.state_requirement, StateRequirement)
        assert isinstance(definition.mutating, bool)
        assert definition.description == definition.callable.__doc__
        assert Path(definition.documentation_anchor.split("#", 1)[0]).is_file()
        assert Path(definition.transport_test).is_file()
        if definition.sensitive_engine_behavior is not SensitiveEngineBehavior.NOT_APPLICABLE:
            assert definition.policy_gate is True


def test_registry_validation_rejects_duplicate_names(monkeypatch: pytest.MonkeyPatch) -> None:
    import slopsearx.mcp.tool_registry as registry

    duplicate = replace(TOOL_DEFINITIONS[0], contract="slopsearx.duplicate")
    monkeypatch.setattr(registry, "TOOL_DEFINITIONS", (*TOOL_DEFINITIONS, duplicate))
    with pytest.raises(ValueError, match="duplicate MCP tool definitions"):
        validate_tool_registry()


async def test_fastmcp_discovery_is_registry_ordered_and_described() -> None:
    discovered = await create_server().list_tools()
    assert tuple(tool.name for tool in discovered) == tool_names()
    for tool, definition in zip(discovered, TOOL_DEFINITIONS, strict=True):
        assert tool.description == definition.description
