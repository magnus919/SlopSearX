"""Cost- and coverage-aware routing strategy (issue 192).

Extends the deterministic keyword router (:mod:`slopsearx.router`) with a
bounded, fully explainable selection pass over the live capability catalog.
The automatic routing paths in :class:`~slopsearx.service.ScopeResolver`
feed their candidate engine sets through this module whenever a capability
catalog is available; explicit ``engines`` lists and targeted searches never
enter this pass.

Routing inputs and precedence (issue 192 scope):

1. **policy eligibility** — disabled/sensitive engines are excluded by the
   caller (:class:`~slopsearx.service.ScopeResolver`) before this module runs;
2. **capability fit** — the candidate set is already filtered to engines that
   serve the requested categories, topic, or evidence intent;
3. **authentication/readiness** — an engine that declares ``auth_class``
   ``required`` without configured credentials is excluded (stage ``auth``);
4. **current health** — a circuit-open engine is excluded (stage ``health``);
5. **cost class** — engines beyond the configured budget's
   ``max_cost_class`` are excluded (stage ``budget``);
6. **evidence coverage** — the scope is capped at the budget's
   ``max_engines`` bound, preferring cheap healthy engines, and coverage
   shortfalls are reported as trade-offs.

Every exclusion and trade-off is recorded with a machine-readable reason so
the scope preview and result metadata can explain the decision. When the
catalog or an individual engine's telemetry is missing, stale, or
unavailable the selection is *conservative*: candidates we cannot assess are
kept, never speculatively dropped — a deterministic choice, not an
optimization.

All selection is pure and deterministic: identical inputs always produce
identical output, so evaluation against a baseline is reproducible.
"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from typing import Any

from slopsearx.capabilities import AUTH_REQUIRED, CapabilityCatalog
from slopsearx.config import Config, load_config

# Cost-class ordering for deterministic comparisons. ``""`` (unknown) maps
# to 0 so the ``max_cost_class`` exclusion never penalizes an unassessed
# engine; the engine-count-cap ordering remaps it above "paid" (see
# ``_priority_order``) so the cap never prefers an unassessed engine over a
# declared-cheaper one. Declared classes: "free" < "freemium" < "paid".
_COST_RANK: dict[str, int] = {"": 0, "free": 1, "freemium": 2, "paid": 3}

# Observed-health ordering for deterministic prioritization. An engine with
# a fresher, healthier observation is preferred under an engine-count bound.
_HEALTH_RANK: dict[str, int] = {
    "ok": 0,
    "unknown": 1,
    "rate_limited": 2,
    "blocked": 3,
    "timeout": 4,
    "error": 5,
    "unavailable": 6,
}

# Machine-readable exclusion-stage vocabulary (closed set).
EXCLUSION_STAGE_POLICY = "policy"
EXCLUSION_STAGE_AUTH = "auth"
EXCLUSION_STAGE_HEALTH = "health"
EXCLUSION_STAGE_BUDGET = "budget"

# Machine-readable trade-off kinds (closed set).
TRADEOFF_COST = "cost"
TRADEOFF_AVAILABILITY = "availability"
TRADEOFF_COVERAGE = "coverage"

# Operator routing-bounds environment variables (win over the YAML section).
ENV_MAX_ENGINES = "ROUTING_MAX_ENGINES_PER_INTENT"
ENV_MAX_COST_CLASS = "ROUTING_MAX_COST_CLASS"
ENV_COVERAGE_TARGET = "ROUTING_COVERAGE_TARGET"


@dataclass(frozen=True)
class RoutingBudget:
    """Operator-configured bounds for the automatic source mix.

    Every bound is permissive by default: ``max_engines`` 0 means no
    engine-count cap, ``max_cost_class`` ``"paid"`` allows every declared
    cost class, and ``coverage_target`` 0 disables coverage reporting.
    Bounds only bite when the operator configures them — autonomous
    spending or hidden provider preference is never inferred (out of scope).
    """

    max_engines: int = 0  # 0 = unlimited
    max_cost_class: str = "paid"
    coverage_target: int = 0


@dataclass(frozen=True)
class RoutingSignal:
    """Per-engine routing inputs derived from the live capability catalog."""

    name: str
    auth_ready: bool
    circuit_open: bool
    cost_class: str
    health_status: str = "unknown"
    health_stale: bool = False
    telemetry_missing: bool = False


@dataclass
class RoutingExclusion:
    """One engine excluded from the automatic scope, with a machine-readable reason."""

    engine: str
    stage: str
    reason: str


@dataclass
class RoutingTradeoff:
    """A reported trade-off between evidence coverage and cost/availability."""

    kind: str
    detail: str


@dataclass
class RoutingSelection:
    """The outcome of one cost/coverage-aware selection pass."""

    selected: list[str]
    exclusions: list[RoutingExclusion] = field(default_factory=list)
    tradeoffs: list[RoutingTradeoff] = field(default_factory=list)
    budget_applied: bool = False


def routing_inputs() -> list[str]:
    """The routing inputs in precedence order (documentation/explainability)."""
    return [
        "policy eligibility",
        "capability fit",
        "authentication/readiness",
        "current health",
        "cost class",
        "evidence coverage",
    ]


def load_routing_budget(config: Config | None = None) -> RoutingBudget:
    """Load the routing budget from the ``routing.budget`` YAML section + env.

    Reads ``max_engines_per_intent``, ``max_cost_class``, and
    ``coverage_target`` with ``ROUTING_*`` env overrides (env wins).
    Invalid or unknown values are ignored (permissive defaults), so a typo
    can never invent a bound.
    """
    config = config or load_config()
    budget_cfg: dict[str, Any] = {}
    raw = config.routing.budget
    if isinstance(raw, dict):
        budget_cfg = dict(raw)
    max_engines = _coerce_non_negative_int(budget_cfg.get("max_engines_per_intent"), ENV_MAX_ENGINES)
    coverage_target = _coerce_non_negative_int(budget_cfg.get("coverage_target"), ENV_COVERAGE_TARGET)
    max_cost_class = _coerce_cost_class(budget_cfg.get("max_cost_class"), ENV_MAX_COST_CLASS)
    return RoutingBudget(
        max_engines=max_engines,
        max_cost_class=max_cost_class,
        coverage_target=coverage_target,
    )


def build_routing_signals(names: list[str], catalog: CapabilityCatalog) -> dict[str, RoutingSignal]:
    """Derive per-engine routing signals from the live catalog.

    An engine with no catalog entry (missing telemetry) yields a permissive
    signal (``auth_ready=True``, circuit closed) so it is kept — a
    conservative deterministic choice, never a speculative drop.
    """
    signals: dict[str, RoutingSignal] = {}
    for name in names:
        cap = catalog.get(name)
        if cap is None:
            signals[name] = RoutingSignal(
                name=name,
                auth_ready=True,
                circuit_open=False,
                cost_class="",
                telemetry_missing=True,
            )
            continue
        signals[name] = RoutingSignal(
            name=name,
            auth_ready=not (cap.auth_class == AUTH_REQUIRED and not cap.auth_configured),
            circuit_open=cap.circuit_open,
            cost_class=cap.cost_class,
            health_status=cap.last_known_status,
            health_stale=cap.last_known_status_stale,
        )
    return signals


def select_cost_coverage(
    candidates: list[str],
    catalog: CapabilityCatalog,
    budget: RoutingBudget | None = None,
    *,
    tier1: set[str] | frozenset[str] | None = None,
) -> RoutingSelection:
    """Select a bounded, cheap, healthy source mix from ``candidates``.

    Deterministic: identical inputs always produce identical output.

    Hard exclusions always apply when the catalog is present:
    unauthenticated (``auth``) and circuit-open (``health``) engines are
    never selected. When a budget is supplied, the cost bound
    (``max_cost_class``) and the engine-count bound (``max_engines``) may
    further exclude engines, and every resulting coverage shortfall is
    reported as a machine-readable trade-off.
    """
    tier1_set = set(tier1 or frozenset())
    signals = build_routing_signals(candidates, catalog)

    selected: list[str] = []
    exclusions: list[RoutingExclusion] = []
    for name in candidates:
        sig = signals[name]
        if not sig.auth_ready:
            exclusions.append(
                RoutingExclusion(
                    engine=name,
                    stage=EXCLUSION_STAGE_AUTH,
                    reason="engine requires credentials that are not configured",
                )
            )
            continue
        if sig.circuit_open:
            exclusions.append(
                RoutingExclusion(
                    engine=name,
                    stage=EXCLUSION_STAGE_HEALTH,
                    reason="engine circuit is open; dispatch is skipped until it recovers",
                )
            )
            continue
        selected.append(name)

    budget_applied = False
    if budget is not None:
        kept: list[str] = []
        for name in selected:
            if _cost_rank(signals[name].cost_class) > _cost_rank(budget.max_cost_class):
                exclusions.append(
                    RoutingExclusion(
                        engine=name,
                        stage=EXCLUSION_STAGE_BUDGET,
                        reason=(
                            f"cost_class '{signals[name].cost_class or 'unknown'}' "
                            f"exceeds the configured budget max '{budget.max_cost_class}'"
                        ),
                    )
                )
                budget_applied = True
            else:
                kept.append(name)
        selected = kept

        if budget.max_engines > 0 and len(selected) > budget.max_engines:
            ordered = _priority_order(selected, signals, tier1_set)
            selected = ordered[: budget.max_engines]
            for name in ordered[budget.max_engines :]:
                exclusions.append(
                    RoutingExclusion(
                        engine=name,
                        stage=EXCLUSION_STAGE_BUDGET,
                        reason=f"scope exceeds the configured per-intent engine budget ({budget.max_engines})",
                    )
                )
            budget_applied = True

    tradeoffs: list[RoutingTradeoff] = []
    if any(ex.stage == EXCLUSION_STAGE_BUDGET for ex in exclusions):
        tradeoffs.append(
            RoutingTradeoff(
                kind=TRADEOFF_COST,
                detail=(
                    "coverage is bounded by the configured routing budget; engines were excluded for cost or count"
                ),
            )
        )
    if any(ex.stage == EXCLUSION_STAGE_HEALTH for ex in exclusions):
        tradeoffs.append(
            RoutingTradeoff(
                kind=TRADEOFF_AVAILABILITY,
                detail="coverage reflects only healthy engines; degraded engines were excluded",
            )
        )
    if budget is not None and budget.coverage_target > 0 and len(selected) < budget.coverage_target:
        tradeoffs.append(
            RoutingTradeoff(
                kind=TRADEOFF_COVERAGE,
                detail=f"selected {len(selected)} engines below the coverage target of {budget.coverage_target}",
            )
        )

    return RoutingSelection(
        selected=selected,
        exclusions=exclusions,
        tradeoffs=tradeoffs,
        budget_applied=budget_applied,
    )


def _priority_order(
    names: list[str],
    signals: dict[str, RoutingSignal],
    tier1: set[str],
) -> list[str]:
    """Deterministic priority order for the engine-count bound.

    Prefers (in order): tier-1 breadth, healthier observation (a stale
    observation is treated as unknown, never as current ``ok``), cheaper
    declared cost class, then stable name order. An unassessed
    (unknown-cost) engine sorts after every declared class under this cap:
    the count bound must never prefer an unassessed engine over a
    declared-free one, while the ``max_cost_class`` exclusion still keeps
    unknown-cost engines.
    """

    def _key(name: str) -> tuple[int, int, int, str]:
        sig = signals[name]
        return (0 if name in tier1 else 1, _health_rank(sig), _cost_rank(sig.cost_class) or 4, name)

    return sorted(names, key=_key)


def _cost_rank(cost_class: str) -> int:
    return _COST_RANK.get(cost_class, 0)


def _health_rank(sig: RoutingSignal) -> int:
    status = "unknown" if sig.health_stale else (sig.health_status if sig.health_status in _HEALTH_RANK else "unknown")
    return _HEALTH_RANK.get(status, 1)


def _coerce_non_negative_int(value: Any, env_var: str) -> int:
    raw = os.environ.get(env_var, "").strip()
    if raw:
        value = raw
    if isinstance(value, bool):
        return 0
    try:
        parsed = int(value)
    except (TypeError, ValueError):
        return 0
    return parsed if parsed > 0 else 0


def _coerce_cost_class(value: Any, env_var: str) -> str:
    raw = os.environ.get(env_var, "").strip()
    if raw:
        value = raw
    if isinstance(value, str) and value in _COST_RANK:
        return value
    return "paid"
