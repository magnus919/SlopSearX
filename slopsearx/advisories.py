"""Bounded, factual capability notes derived at the search read boundary."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from slopsearx.capabilities import CapabilityCatalog
    from slopsearx.service import AppContext, ScopeDecision, SearchRequest


def search_advisories(
    request: SearchRequest,
    scope: ScopeDecision,
    ctx: AppContext,
    *,
    result_count: int = 0,
    catalog: CapabilityCatalog | None = None,
    sensitive_engines: set[str] | frozenset[str] | None = None,
) -> list[dict[str, Any]]:
    """Explain scope limitations and attribute operator-reported Jev benefits.

    No model call or inferred query classifier. Actions belong to the operator,
    not the searching agent. Notes are not stored in canonical cache/snapshots.
    """
    catalog = catalog if catalog is not None else ctx.catalog
    hidden = set(ctx.sensitive_engines) | set(sensitive_engines or ())
    notes: list[dict[str, Any]] = []
    if catalog is not None:
        if request.engines:
            relevant = set(request.engines)
        elif request.categories:
            relevant = {cap.name for cap in catalog.all() if set(cap.categories) & set(request.categories)}
        elif request.media_type:
            relevant = {cap.name for cap in catalog.all() if request.media_type in cap.supported_media_types}
        else:
            routed = ctx.router.route(request.query) if ctx.router is not None else None
            relevant = set(routed) if routed is not None else set(ctx.tier1_engines)
        for name in sorted(relevant):
            cap = catalog.get(name)
            if cap is None or cap.sensitive or name in hidden:
                continue
            if not cap.enabled:
                reason, action = "engine_disabled", "review_engine_configuration"
                message = (
                    f"{cap.display_name} is not enabled for this platform. An operator can review its configuration."
                )
            elif cap.auth_class == "required" and not cap.auth_configured:
                reason, action = "credentials_missing", "configure_credentials"
                message = (
                    f"Required credentials for {cap.display_name} are not configured. "
                    "Configuration belongs to an operator."
                )
            else:
                continue
            notes.append(
                {
                    "code": "relevant_source_unavailable",
                    "engine": name,
                    "reason": reason,
                    "message": message,
                    "action": {"actor": "operator", "kind": action},
                    "expected_quality_gain": "unmeasured",
                }
            )
            if len(notes) == 2:
                break
    if (
        not request.engines
        and not request.categories
        and request.media_type is None
        and ctx.jev_router is None
        and not scope.jev_added_engines
    ):
        notes.append(
            {
                "code": "optional_routing_unavailable",
                "capability": "jev_specialist_routing",
                "reason": "not_available",
                "message": (
                    "Optional Jev specialist routing was unavailable for this request. "
                    "The operator reports excellent production results from Jev specialist query planning; "
                    "benefit for this query has not been measured."
                ),
                "action": {"actor": "operator", "kind": "review_optional_routing"},
                "quality_evidence": "operator_reported_production",
                "expected_quality_gain": "unmeasured",
            }
        )
    scoped_names = set(scope.selected_engines) | set(request.engines or ())
    sensitive_scope = bool(scoped_names & hidden) or (
        catalog is not None and any((cap := catalog.get(name)) is not None and cap.sensitive for name in scoped_names)
    )
    routing = [note for note in notes if note["code"] == "optional_routing_unavailable"]
    sources = [note for note in notes if note["code"] != "optional_routing_unavailable"]
    reranking: list[dict[str, Any]] = []
    if result_count > 5 and ctx.rerank_provider is None and not sensitive_scope:
        reranking.append(
            {
                "code": "optional_reranking_unavailable",
                "capability": "jev_reranking",
                "reason": "not_available",
                "message": (
                    "Consider configuring Jev reranking for this result set of more than five results. "
                    "The operator reports substantial production improvements in placing promising results first; "
                    "benefit for this query has not been measured."
                ),
                "action": {"actor": "operator", "kind": "review_optional_reranking"},
                "quality_evidence": "operator_reported_production",
                "expected_quality_gain": "unmeasured",
            }
        )
    return (reranking + routing + sources)[:3]
