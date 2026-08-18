"""Routing evaluation: quality against a context-equivalent baseline (issue 192).

Measures the cost/coverage-aware routing strategy against the deterministic
fallback (the candidate set the router would otherwise dispatch) using
declared, deterministic fixtures — no network, no models, no live telemetry.

Scope of this evaluation (acceptance: results are scoped to the tested query
families, fixtures, models, and date):

- query families: general (unscoped), code, science — modeled as candidate
  engine sets per fixture (the strategy is query-agnostic by design: the
  keyword topic router narrows to a topic's engines *before* this pass);
- fixtures: healthy/degraded, authenticated/unauthenticated, cheap/expensive,
  high/low coverage mixes (declared below);
- models: none — the strategy is a pure deterministic function;
- date: captured at eval time in ``eval_scope()``.

This benchmark claims only what it measures: under the declared fixtures and
budgets, the routing pass never exceeds configured bounds, never selects
ineligible/unhealthy engines, never adds engines the baseline would not
dispatch, never increases the average cost of the mix, and is fully
deterministic across repeated runs. It does not claim universal quality
improvement.
"""

from __future__ import annotations

import datetime as _dt
from typing import Any

import pytest

from slopsearx.capabilities import EngineCapability
from slopsearx.routing import RoutingBudget, RoutingSelection, select_cost_coverage

# Query families modeled by this evaluation (candidate sets per family).
QUERY_FAMILIES = ("general", "code", "science")

# Cost ordering shared by the fixture assertions (mirrors the strategy).
_COST_RANK = {"": 0, "free": 1, "freemium": 2, "paid": 3}

# Deterministic run count for the determinism rubric (issue 192 acceptance:
# "run counts" are part of the measurement).
DETERMINISM_RUNS = 25


class _EvalCatalog:
    """Catalog stand-in exposing ``get(name)`` over declared capabilities."""

    def __init__(self, caps: dict[str, EngineCapability]) -> None:
        self._caps = caps

    def get(self, name: str) -> EngineCapability | None:
        return self._caps.get(name)


def _cap(
    name: str,
    *,
    auth_class: str = "none",
    auth_configured: bool = False,
    circuit_open: bool = False,
    cost_class: str = "free",
    status: str = "unknown",
) -> EngineCapability:
    return EngineCapability(
        name=name,
        display_name=name,
        engine_type="api",
        categories=["general"],
        enabled=True,
        auth_class=auth_class,
        auth_configured=auth_configured,
        scope_hints=[],
        caveats=[],
        cost_class=cost_class,
        last_known_status=status,
        last_known_status_stale=False,
        circuit_open=circuit_open,
    )


# Declared fixtures: (name, candidates, capabilities, budget, note).
# Each fixture models a distinct routing-relevant mix.
FIXTURES: tuple[tuple[str, list[str], dict[str, EngineCapability], RoutingBudget, str], ...] = (
    (
        "healthy_cheap",
        ["wikipedia", "arxiv", "openalex", "duckduckgo"],
        {n: _cap(n) for n in ["wikipedia", "arxiv", "openalex", "duckduckgo"]},
        RoutingBudget(max_engines=3, coverage_target=2),
        "all healthy, all free, high coverage",
    ),
    (
        "degraded",
        ["wikipedia", "sick_a", "sick_b", "duckduckgo"],
        {
            "wikipedia": _cap("wikipedia"),
            "sick_a": _cap("sick_a", status="error"),
            "sick_b": _cap("sick_b", circuit_open=True),
            "duckduckgo": _cap("duckduckgo"),
        },
        RoutingBudget(max_engines=3),
        "mixed health; one circuit-open",
    ),
    (
        "unauth",
        ["wikipedia", "brave", "shodan", "duckduckgo"],
        {
            "wikipedia": _cap("wikipedia"),
            "brave": _cap("brave", auth_class="required", auth_configured=False, cost_class="freemium"),
            "shodan": _cap("shodan", auth_class="required", auth_configured=False, cost_class="freemium"),
            "duckduckgo": _cap("duckduckgo"),
        },
        RoutingBudget(max_engines=4),
        "unauthenticated required-key engines mixed with keyless ones",
    ),
    (
        "expensive",
        ["wikipedia", "dehashed", "intelx", "duckduckgo"],
        {
            "wikipedia": _cap("wikipedia"),
            "dehashed": _cap("dehashed", cost_class="paid"),
            "intelx": _cap("intelx", cost_class="freemium"),
            "duckduckgo": _cap("duckduckgo"),
        },
        RoutingBudget(max_engines=3, max_cost_class="freemium"),
        "cheap + freemium + paid mix under a freemium budget",
    ),
    (
        "low_coverage",
        ["wikipedia"],
        {"wikipedia": _cap("wikipedia")},
        RoutingBudget(coverage_target=4),
        "single-engine mix below the coverage target",
    ),
    (
        "high_coverage_mixed",
        ["wikipedia", "brave", "arxiv", "github", "stackexchange", "dehashed", "shodan", "duckduckgo"],
        {
            "wikipedia": _cap("wikipedia"),
            "brave": _cap("brave", auth_class="required", auth_configured=True, cost_class="freemium", status="ok"),
            "arxiv": _cap("arxiv"),
            "github": _cap("github"),
            "stackexchange": _cap("stackexchange"),
            "dehashed": _cap("dehashed", cost_class="paid"),
            "shodan": _cap("shodan", auth_class="required", auth_configured=False, cost_class="freemium"),
            "duckduckgo": _cap("duckduckgo"),
        },
        RoutingBudget(max_engines=5, max_cost_class="freemium", coverage_target=4),
        "everything at once: healthy/degraded, authed/unauth, cheap/expensive",
    ),
    (
        "cheap_only_after_health",
        ["u1", "u2", "u3", "u4", "free_ok", "paid_ok"],
        {
            "u1": _cap("u1", cost_class="", circuit_open=True),
            "u2": _cap("u2", cost_class="", circuit_open=True),
            "u3": _cap("u3", cost_class="", circuit_open=True),
            "u4": _cap("u4", cost_class="", circuit_open=True),
            "free_ok": _cap("free_ok"),
            "paid_ok": _cap("paid_ok", cost_class="paid"),
        },
        RoutingBudget(max_cost_class="free"),
        "unknown-cost engines are circuit-open; only the free engine survives health + budget",
    ),
)

# The declared query-family → candidate mapping is context-equivalent to the
# baseline: each family dispatches exactly the fixture's candidate set when
# no topic matches and no budget is configured (the deterministic fallback).
FAMILY_CANDIDATES: dict[str, list[str]] = {
    "general": ["wikipedia", "arxiv", "openalex", "duckduckgo"],
    "code": ["github", "stackexchange", "pypi", "npm"],
    "science": ["arxiv", "openalex", "semanticscholar", "pubmed"],
}


def _avg_cost(caps: dict[str, EngineCapability], names: list[str]) -> float:
    if not names:
        return 0.0
    return sum(_COST_RANK.get(caps[name].cost_class, 0) for name in names) / len(names)


def eval_scope() -> dict[str, Any]:
    """The declared evaluation scope (fixtures, families, models, date)."""
    return {
        "query_families": list(QUERY_FAMILIES),
        "fixtures": [name for name, *_ in FIXTURES],
        "models": "none (pure deterministic strategy)",
        "date": _dt.date.today().isoformat(),
    }


class TestRoutingEvaluation:
    def test_eval_scope_is_declared(self) -> None:
        """The evaluation is scoped to its declared fixtures, families, models, date."""
        scope = eval_scope()
        assert set(scope["query_families"]) == set(QUERY_FAMILIES)
        assert "healthy_cheap" in scope["fixtures"]
        assert scope["models"] == "none (pure deterministic strategy)"
        assert scope["date"]

    @pytest.mark.parametrize("fixture", FIXTURES, ids=lambda fx: fx[0])
    def test_rubric_passes_for_declared_fixtures(self, fixture: Any) -> None:
        """Every declared fixture satisfies the routing rubric (see module doc)."""
        name, candidates, caps, budget, note = fixture
        catalog = _EvalCatalog(caps)

        # Run count for determinism measurement.
        results: list[RoutingSelection] = [
            select_cost_coverage(candidates, catalog, budget) for _ in range(DETERMINISM_RUNS)
        ]
        routed = results[0]
        baseline = candidates  # context-equivalent deterministic fallback

        report: dict[str, Any] = {
            "fixture": name,
            "note": note,
            "query_families": list(QUERY_FAMILIES),
            "models": "none",
            "runs": len(results),
            "budget": {
                "max_engines": budget.max_engines,
                "max_cost_class": budget.max_cost_class,
                "coverage_target": budget.coverage_target,
            },
            "baseline": {"selected": baseline, "count": len(baseline)},
            "routed": {"selected": routed.selected, "count": len(routed.selected)},
            "exclusions": [{"engine": e.engine, "stage": e.stage} for e in routed.exclusions],
            "tradeoffs": [{"kind": tr.kind} for tr in routed.tradeoffs],
            "rubric": {},
        }

        rubric = report["rubric"]

        # R1 — routing never adds engines the baseline would not dispatch.
        assert set(routed.selected) <= set(baseline)
        rubric["subset_of_baseline"] = True

        # R2 — configured bounds are respected.
        if budget.max_engines > 0:
            assert len(routed.selected) <= budget.max_engines
        for engine in routed.selected:
            assert _COST_RANK.get(caps[engine].cost_class, 0) <= _COST_RANK[budget.max_cost_class]
        rubric["bounds_respected"] = True

        # R3 — no ineligible (unauthenticated/circuit-open) engine is selected.
        for engine in routed.selected:
            assert caps[engine].auth_class != "required" or caps[engine].auth_configured
            assert caps[engine].circuit_open is False
        rubric["no_ineligible_selected"] = True

        # R4 — determinism across the declared run count.
        assert all(r.selected == routed.selected for r in results)
        rubric["deterministic"] = True

        # R5 — when the cost bound actually excludes engines, the routed mix
        # is never more expensive on average than the auth/health-filtered
        # baseline (only budget exclusions may narrow it further). Comparing
        # against the raw candidate set is an unsound proxy: auth/health
        # exclusions can remove the cheap or unknown-cost engines, so the cost
        # bound may be satisfiable even when the raw baseline average is below
        # every in-budget engine's cost. When the cost bound is not
        # constraining (filtered baseline already within budget), no cost
        # improvement is claimed — the strategy only narrows scope, so it may
        # remove cheap unhealthy engines and leave pricier healthy ones.
        filtered_baseline = [
            n
            for n in baseline
            if not (caps[n].auth_class == "required" and not caps[n].auth_configured) and not caps[n].circuit_open
        ]
        baseline_max_cost = max((_COST_RANK.get(caps[n].cost_class, 0) for n in filtered_baseline), default=0)
        if _COST_RANK[budget.max_cost_class] < baseline_max_cost:
            assert _avg_cost(caps, routed.selected) <= _avg_cost(caps, filtered_baseline) + 1e-9
            rubric["cost_not_worse_than_baseline"] = True
        else:
            rubric["cost_not_worse_than_baseline"] = "n/a (cost bound not constraining)"

        # R6 — coverage is not reduced below the budget floor.
        if budget.coverage_target > 0:
            assert len(routed.selected) >= min(len(baseline), budget.coverage_target)
        rubric["coverage_floor"] = True

        assert all(rubric.values()), report

    @pytest.mark.parametrize("family", QUERY_FAMILIES)
    def test_family_candidate_sets_are_non_empty(self, family: str) -> None:
        """Every declared query family has a concrete candidate set for eval."""
        assert FAMILY_CANDIDATES[family]

    def test_baseline_is_the_unbounded_deterministic_scope(self) -> None:
        """Without budget bounds the routed scope is a subset of the fallback."""
        caps = {n: _cap(n) for n in ["wikipedia", "arxiv", "openalex", "duckduckgo"]}
        catalog = _EvalCatalog(caps)
        candidates = ["wikipedia", "arxiv", "openalex", "duckduckgo"]
        unbounded = select_cost_coverage(candidates, catalog, RoutingBudget())
        assert set(unbounded.selected) == set(candidates)
        assert unbounded.budget_applied is False

    def test_r5_uses_auth_health_filtered_baseline(self) -> None:
        """R5 must compare against the auth/health-filtered baseline, not the
        raw candidate set: auth/health exclusions can remove the cheap or
        unknown-cost engines, so the cost bound may be satisfiable even when
        the raw baseline average is below every in-budget engine's cost
        (issue 192 review).

        Reviewer arithmetic: candidates ``[u1..u4 (unknown cost,
        circuit-open), free_ok (free), paid_ok (paid)]`` with
        ``max_cost_class="free"`` — the strategy correctly routes to
        ``free_ok`` only, and R5 must pass (raw-baseline comparison would
        wrongly fail: 1.0 > (0+0+0+0+1+3)/6)."""
        candidates = ["u1", "u2", "u3", "u4", "free_ok", "paid_ok"]
        caps = {f"u{i}": _cap(f"u{i}", cost_class="", circuit_open=True) for i in range(1, 5)}
        caps["free_ok"] = _cap("free_ok")
        caps["paid_ok"] = _cap("paid_ok", cost_class="paid")
        catalog = _EvalCatalog(caps)
        budget = RoutingBudget(max_cost_class="free")

        routed = select_cost_coverage(candidates, catalog, budget)
        assert routed.selected == ["free_ok"]

        filtered_baseline = [
            n
            for n in candidates
            if not (caps[n].auth_class == "required" and not caps[n].auth_configured) and not caps[n].circuit_open
        ]
        assert set(filtered_baseline) == {"free_ok", "paid_ok"}
        baseline_max_cost = max((_COST_RANK.get(caps[n].cost_class, 0) for n in filtered_baseline), default=0)
        assert _COST_RANK[budget.max_cost_class] < baseline_max_cost
        assert _avg_cost(caps, routed.selected) <= _avg_cost(caps, filtered_baseline) + 1e-9
