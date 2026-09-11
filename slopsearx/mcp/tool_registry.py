"""Declarative inventory for the public MCP tool surface.

Tool implementations remain ordinary async callables in their owning modules.
This module is the single place that decides which callables are exposed over
FastMCP and records the contract metadata needed to audit that exposure.
Policy enforcement remains in the implementations and their shared
``_enforce_policy`` path; the registry describes that boundary but never
reimplements it.
"""

from __future__ import annotations

import inspect
from collections.abc import Callable
from dataclasses import dataclass
from enum import StrEnum
from typing import Any

from slopsearx.mcp import dependency_tools, receipt_tools, staged_tools, tools


class StateRequirement(StrEnum):
    """Shared-state contract exercised by a tool."""

    NONE = "none"
    SNAPSHOT = "snapshot"
    DURABLE = "durable"


class SensitiveEngineBehavior(StrEnum):
    """How a tool treats engine scopes that may contain sensitive engines."""

    NOT_APPLICABLE = "not_applicable"
    SHARED_GATE_BEFORE_DISPATCH = "shared_gate_before_dispatch"
    STORED_SCOPE_REVALIDATED = "stored_scope_revalidated"


@dataclass(frozen=True, slots=True)
class ToolDefinition:
    """One stable MCP tool registration and its audited contract metadata."""

    callable: Callable[..., Any]
    name: str
    owner_module: str
    contract: str
    contract_version: str
    required_grants: tuple[str, ...]
    state_requirement: StateRequirement
    mutating: bool
    sensitive_engine_behavior: SensitiveEngineBehavior
    policy_gate: bool
    description: str
    documentation_anchor: str
    transport_test: str


def _definition(
    callable_: Callable[..., Any],
    *,
    contract: str,
    grants: tuple[str, ...] = (),
    state: StateRequirement = StateRequirement.NONE,
    mutating: bool = False,
    sensitive: SensitiveEngineBehavior = SensitiveEngineBehavior.NOT_APPLICABLE,
    policy_gate: bool = False,
    docs: str,
    transport_test: str = "tests/test_mcp_harness.py",
) -> ToolDefinition:
    """Build a definition while rejecting incomplete or renamed callables."""
    name = callable_.__name__
    owner_module = callable_.__module__
    description = callable_.__doc__ or ""
    if not name.startswith("slopsearx_"):
        raise ValueError(f"MCP tool name must start with slopsearx_: {name}")
    if not owner_module.startswith("slopsearx.mcp."):
        raise ValueError(f"MCP tool must be owned by slopsearx.mcp: {owner_module}.{name}")
    if not contract or not description or not docs or not transport_test:
        raise ValueError(f"MCP tool metadata is incomplete: {name}")
    return ToolDefinition(
        callable=callable_,
        name=name,
        owner_module=owner_module,
        contract=contract,
        contract_version="1.0",
        required_grants=grants,
        state_requirement=state,
        mutating=mutating,
        sensitive_engine_behavior=sensitive,
        policy_gate=policy_gate,
        description=description,
        documentation_anchor=docs,
        transport_test=transport_test,
    )


_SEARCH_GATE = SensitiveEngineBehavior.SHARED_GATE_BEFORE_DISPATCH
_REVALIDATED_SCOPE = SensitiveEngineBehavior.STORED_SCOPE_REVALIDATED


TOOL_DEFINITIONS: tuple[ToolDefinition, ...] = (
    _definition(
        tools.slopsearx_search,
        contract="slopsearx.search",
        state=StateRequirement.SNAPSHOT,
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#61-slopsearx_search",
    ),
    _definition(
        tools.slopsearx_search_targeted,
        contract="slopsearx.search",
        state=StateRequirement.SNAPSHOT,
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#62-slopsearx_search_targeted",
    ),
    _definition(
        tools.slopsearx_search_jobs,
        contract="slopsearx.search.jobs",
        grants=("jobs",),
        state=StateRequirement.SNAPSHOT,
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#63-slopsearx_search_jobs-grant-mcp_grant_jobs",
    ),
    _definition(
        tools.slopsearx_search_security,
        contract="slopsearx.search.security",
        grants=("security",),
        state=StateRequirement.SNAPSHOT,
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#64-slopsearx_search_security-grant-mcp_grant_security",
    ),
    _definition(
        tools.slopsearx_search_science,
        contract="slopsearx.search.science",
        grants=("science",),
        state=StateRequirement.SNAPSHOT,
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#65-slopsearx_search_science-grant-mcp_grant_science",
    ),
    _definition(
        tools.slopsearx_list_capabilities,
        contract="slopsearx.capabilities",
        docs="docs/MCP_SERVER.md#66-slopsearx_list_capabilities",
    ),
    _definition(
        tools.slopsearx_explain_search_scope,
        contract="slopsearx.search.scope",
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#67-slopsearx_explain_search_scope",
    ),
    _definition(
        tools.slopsearx_get_service_status,
        contract="slopsearx.service_status",
        docs="docs/MCP_SERVER.md#68-slopsearx_get_service_status",
    ),
    _definition(
        tools.slopsearx_read_results,
        contract="slopsearx.snapshot.results",
        state=StateRequirement.SNAPSHOT,
        docs="docs/MCP_SERVER.md#69-slopsearx_read_results--610-slopsearx_read_result",
    ),
    _definition(
        tools.slopsearx_read_result,
        contract="slopsearx.snapshot.result",
        state=StateRequirement.SNAPSHOT,
        docs="docs/MCP_SERVER.md#69-slopsearx_read_results--610-slopsearx_read_result",
    ),
    _definition(
        tools.slopsearx_read_entities,
        contract="slopsearx.snapshot.entities",
        state=StateRequirement.SNAPSHOT,
        docs="docs/ENTITY_GROUPING.md",
        transport_test="tests/test_entity_transport.py",
    ),
    _definition(
        tools.slopsearx_start_research,
        contract="slopsearx.research",
        grants=("research",),
        state=StateRequirement.DURABLE,
        mutating=True,
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#611-616-research-jobs-grant-mcp_grant_research",
        transport_test="tests/test_adaptive_transport.py",
    ),
    _definition(
        tools.slopsearx_get_job,
        contract="slopsearx.research",
        state=StateRequirement.DURABLE,
        docs="docs/MCP_SERVER.md#611-616-research-jobs-grant-mcp_grant_research",
        transport_test="tests/test_adaptive_transport.py",
    ),
    _definition(
        tools.slopsearx_cancel_job,
        contract="slopsearx.research",
        state=StateRequirement.DURABLE,
        mutating=True,
        docs="docs/MCP_SERVER.md#611-616-research-jobs-grant-mcp_grant_research",
        transport_test="tests/test_adaptive_transport.py",
    ),
    _definition(
        tools.slopsearx_retry_research,
        contract="slopsearx.research",
        grants=("research",),
        state=StateRequirement.DURABLE,
        mutating=True,
        sensitive=_REVALIDATED_SCOPE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#611-616-research-jobs-grant-mcp_grant_research",
        transport_test="tests/test_adaptive_transport.py",
    ),
    _definition(
        tools.slopsearx_extend_research,
        contract="slopsearx.research",
        grants=("research",),
        state=StateRequirement.DURABLE,
        mutating=True,
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/ADAPTIVE_RESEARCH.md",
        transport_test="tests/test_adaptive_transport.py",
    ),
    _definition(
        tools.slopsearx_update_research,
        contract="slopsearx.research",
        grants=("research",),
        state=StateRequirement.DURABLE,
        mutating=True,
        docs="docs/ADAPTIVE_RESEARCH.md",
        transport_test="tests/test_adaptive_transport.py",
    ),
    _definition(
        tools.slopsearx_create_saved_search,
        contract="slopsearx.saved_search",
        grants=("saved_searches",),
        state=StateRequirement.DURABLE,
        mutating=True,
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#6133-saved-searches-grant-mcp_grant_saved_searches",
        transport_test="tests/test_saved_transport.py",
    ),
    _definition(
        tools.slopsearx_get_saved_search,
        contract="slopsearx.saved_search",
        grants=("saved_searches",),
        state=StateRequirement.DURABLE,
        sensitive=_REVALIDATED_SCOPE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#6133-saved-searches-grant-mcp_grant_saved_searches",
        transport_test="tests/test_saved_transport.py",
    ),
    _definition(
        tools.slopsearx_update_saved_search,
        contract="slopsearx.saved_search",
        grants=("saved_searches",),
        state=StateRequirement.DURABLE,
        mutating=True,
        sensitive=_REVALIDATED_SCOPE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#6133-saved-searches-grant-mcp_grant_saved_searches",
        transport_test="tests/test_saved_transport.py",
    ),
    _definition(
        tools.slopsearx_pause_saved_search,
        contract="slopsearx.saved_search",
        grants=("saved_searches",),
        state=StateRequirement.DURABLE,
        mutating=True,
        docs="docs/MCP_SERVER.md#6133-saved-searches-grant-mcp_grant_saved_searches",
        transport_test="tests/test_saved_transport.py",
    ),
    _definition(
        tools.slopsearx_delete_saved_search,
        contract="slopsearx.saved_search",
        grants=("saved_searches",),
        state=StateRequirement.DURABLE,
        mutating=True,
        docs="docs/MCP_SERVER.md#6133-saved-searches-grant-mcp_grant_saved_searches",
        transport_test="tests/test_saved_transport.py",
    ),
    _definition(
        tools.slopsearx_read_saved_search_reports,
        contract="slopsearx.saved_search.reports",
        grants=("saved_searches",),
        state=StateRequirement.DURABLE,
        sensitive=_REVALIDATED_SCOPE,
        policy_gate=True,
        docs="docs/MCP_SERVER.md#6133-saved-searches-grant-mcp_grant_saved_searches",
        transport_test="tests/test_saved_transport.py",
    ),
    _definition(
        receipt_tools.slopsearx_submit_retrieval_receipt,
        contract="slopsearx.retrieval_receipt",
        grants=("retrieval_receipts",),
        state=StateRequirement.DURABLE,
        mutating=True,
        docs="docs/RETRIEVAL_HANDOFF.md",
        transport_test="tests/test_retrieval_receipts_transport.py",
    ),
    _definition(
        receipt_tools.slopsearx_read_retrieval_receipts,
        contract="slopsearx.retrieval_receipt",
        grants=("retrieval_receipts",),
        state=StateRequirement.DURABLE,
        docs="docs/RETRIEVAL_HANDOFF.md",
        transport_test="tests/test_retrieval_receipts_transport.py",
    ),
    _definition(
        receipt_tools.slopsearx_export_research_manifest,
        contract="slopsearx.research_manifest",
        grants=("retrieval_receipts",),
        state=StateRequirement.DURABLE,
        docs="docs/RETRIEVAL_HANDOFF.md",
        transport_test="tests/test_retrieval_receipts_transport.py",
    ),
    _definition(
        staged_tools.slopsearx_preview_staged_search,
        contract="slopsearx.staged_search",
        grants=("staged_search",),
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/STAGED_SEARCH.md",
        transport_test="tests/test_staged_search.py",
    ),
    _definition(
        staged_tools.slopsearx_search_staged,
        contract="slopsearx.staged_search",
        grants=("staged_search",),
        state=StateRequirement.DURABLE,
        mutating=True,
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/STAGED_SEARCH.md",
        transport_test="tests/test_staged_search.py",
    ),
    _definition(
        staged_tools.slopsearx_get_staged_search,
        contract="slopsearx.staged_search",
        grants=("staged_search",),
        state=StateRequirement.DURABLE,
        sensitive=_REVALIDATED_SCOPE,
        policy_gate=True,
        docs="docs/STAGED_SEARCH.md",
        transport_test="tests/test_staged_search.py",
    ),
    _definition(
        staged_tools.slopsearx_retry_staged_search,
        contract="slopsearx.staged_search",
        grants=("staged_search",),
        state=StateRequirement.DURABLE,
        mutating=True,
        sensitive=_REVALIDATED_SCOPE,
        policy_gate=True,
        docs="docs/STAGED_SEARCH.md",
        transport_test="tests/test_staged_search.py",
    ),
    _definition(
        dependency_tools.slopsearx_start_dependency_dossier,
        contract="slopsearx.dependency_dossier",
        grants=("dependency_dossier", "research", "security"),
        state=StateRequirement.DURABLE,
        mutating=True,
        sensitive=_SEARCH_GATE,
        policy_gate=True,
        docs="docs/DEPENDENCY_DOSSIER.md",
        transport_test="tests/test_dependency_dossier.py",
    ),
    _definition(
        dependency_tools.slopsearx_get_dependency_dossier,
        contract="slopsearx.dependency_dossier",
        grants=("dependency_dossier", "research", "security"),
        state=StateRequirement.DURABLE,
        sensitive=_REVALIDATED_SCOPE,
        policy_gate=True,
        docs="docs/DEPENDENCY_DOSSIER.md",
        transport_test="tests/test_dependency_dossier.py",
    ),
)


def tool_names() -> tuple[str, ...]:
    """Return the stable registration order advertised by FastMCP."""
    return tuple(definition.name for definition in TOOL_DEFINITIONS)


def validate_tool_registry() -> None:
    """Fail fast if the declarative inventory is internally inconsistent."""
    names = tool_names()
    if len(names) != len(set(names)):
        duplicates = sorted({name for name in names if names.count(name) > 1})
        raise ValueError(f"duplicate MCP tool definitions: {', '.join(duplicates)}")
    for definition in TOOL_DEFINITIONS:
        if not inspect.iscoroutinefunction(definition.callable):
            raise ValueError(f"MCP tool callable must be async: {definition.name}")
        if definition.callable.__name__ != definition.name:
            raise ValueError(f"MCP tool callable renamed after registration: {definition.name}")
        if definition.callable.__module__ != definition.owner_module:
            raise ValueError(f"MCP tool owner changed after registration: {definition.name}")
        if (
            definition.policy_gate is False
            and definition.sensitive_engine_behavior is not SensitiveEngineBehavior.NOT_APPLICABLE
        ):
            raise ValueError(f"sensitive-engine behavior requires a policy gate: {definition.name}")


validate_tool_registry()
